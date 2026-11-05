//! A small table type for the answers of the analyses, with JSON and CSV output.
//!
//! The rules follow what the original Polars version wrote, so that files made by either
//! version look the same: floats are rounded to three decimals (and NaN becomes empty),
//! empty strings are written as `""` in CSV, missing values as nothing, and a byte-order mark
//! starts the file so that Excel reads the Hebrew.

use serde_json::{Map, Number, Value as Json};
use std::io::{self, Write};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}
impl From<f64> for Value {
    fn from(x: f64) -> Self {
        Value::Float(x)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
macro_rules! from_int {
    ($($t:ty),*) => { $(impl From<$t> for Value { fn from(n: $t) -> Self { Value::Int(n as i64) } })* };
}
from_int!(i32, i64, u8, u16, u32, u64, usize);

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => v.into(),
            None => Value::Null,
        }
    }
}

/// Round to three decimals the way Polars does: scale, round half to even, scale back.
pub fn round3(x: f64) -> f64 {
    (x * 1000.0).round_ties_even() / 1000.0
}

impl Value {
    /// For output: floats are rounded to three decimals; NaN and infinity become Null.
    pub fn tidy(self) -> Value {
        match self {
            Value::Float(x) if !x.is_finite() => Value::Null,
            Value::Float(x) => Value::Float(round3(x)),
            other => other,
        }
    }

    pub fn to_json(&self) -> Json {
        match self {
            Value::Null => Json::Null,
            Value::Bool(b) => Json::Bool(*b),
            Value::Int(n) => Json::Number((*n).into()),
            Value::Float(x) => Number::from_f64(*x).map(Json::Number).unwrap_or(Json::Null),
            Value::Str(s) => Json::String(s.clone()),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// A float the way Polars writes it in a CSV: `3.0`, `0.308`, `1e-7`, `1e+21`.
pub fn format_float(x: f64) -> String {
    let text = format!("{:?}", x);
    match text.find('e') {
        Some(at) if !text[at + 1..].starts_with('-') => format!("{}e+{}", &text[..at], &text[at + 1..]),
        _ => text,
    }
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

impl Table {
    pub fn new(columns: &[&str]) -> Self {
        Table { columns: columns.iter().map(|c| c.to_string()).collect(), rows: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Round floats and turn NaN into null, for output that goes to JSON or CSV.
    pub fn tidy(&self) -> Table {
        Table { columns: self.columns.clone(), rows: self.rows.iter().map(|r| r.iter().cloned().map(Value::tidy).collect()).collect() }
    }

    pub fn column(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }

    /// `{"columns": [...], "rows": [[...], ...]}` parts, tidied.
    pub fn to_json_parts(&self) -> (Json, Json) {
        let columns = Json::Array(self.columns.iter().map(|c| Json::String(c.clone())).collect());
        let rows = Json::Array(self.rows.iter().map(|r| Json::Array(r.iter().cloned().map(|v| v.tidy().to_json()).collect())).collect());
        (columns, rows)
    }

    /// CSV for download, with a byte-order mark so that Excel reads the Hebrew.
    pub fn csv_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut writer = RowWriter::new(&mut out, Format::Csv, &self.columns).expect("writing to memory");
        for row in &self.rows {
            writer.write_row(row).expect("writing to memory");
        }
        writer.finish().expect("writing to memory");
        out
    }
}

// --------------------------------------------------------------------------
// Streaming output (so that exporting the whole corpus does not need it all in memory)
// --------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Tsv,
    Json,
    Ndjson,
}

impl Format {
    /// The format that a file name asks for, by its extension.
    pub fn from_extension(path: &str) -> Option<Format> {
        let lower = path.to_lowercase();
        let ext = lower.rsplit('.').next()?;
        match ext {
            "csv" => Some(Format::Csv),
            "tsv" => Some(Format::Tsv),
            "json" => Some(Format::Json),
            "ndjson" | "jsonl" => Some(Format::Ndjson),
            _ => None,
        }
    }
}

pub struct RowWriter<W: Write> {
    out: W,
    format: Format,
    columns: Vec<String>,
    rows: usize,
}

fn csv_field(value: &Value, separator: char, out: &mut String) {
    match value {
        Value::Null => {}
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(n) => out.push_str(&n.to_string()),
        Value::Float(x) => out.push_str(&format_float(*x)),
        Value::Str(s) => {
            let needs_quotes = s.is_empty() || s.contains(separator) || s.contains('"') || s.contains('\n') || s.contains('\r');
            if needs_quotes {
                out.push('"');
                out.push_str(&s.replace('"', "\"\""));
                out.push('"');
            } else {
                out.push_str(s);
            }
        }
    }
}

impl<W: Write> RowWriter<W> {
    /// Starts the output: the header row (CSV, TSV) or the opening bracket (JSON). Files that
    /// Excel will open begin with a byte-order mark.
    pub fn new(out: W, format: Format, columns: &[String]) -> io::Result<Self> {
        Self::with_bom(out, format, columns, true)
    }

    /// Like `new`, saying whether a CSV or TSV starts with a byte-order mark (not wanted on a terminal).
    pub fn with_bom(mut out: W, format: Format, columns: &[String], bom: bool) -> io::Result<Self> {
        match format {
            Format::Csv | Format::Tsv => {
                if bom {
                    out.write_all(b"\xef\xbb\xbf")?;
                }
                let sep = if format == Format::Csv { ',' } else { '\t' };
                let mut line = String::new();
                for (i, c) in columns.iter().enumerate() {
                    if i > 0 {
                        line.push(sep);
                    }
                    csv_field(&Value::Str(c.clone()), sep, &mut line);
                }
                line.push('\n');
                out.write_all(line.as_bytes())?;
            }
            Format::Json => out.write_all(b"[")?,
            Format::Ndjson => {}
        }
        Ok(RowWriter { out, format, columns: columns.to_vec(), rows: 0 })
    }

    pub fn write_row(&mut self, row: &[Value]) -> io::Result<()> {
        match self.format {
            Format::Csv | Format::Tsv => {
                let sep = if self.format == Format::Csv { ',' } else { '\t' };
                let mut line = String::new();
                for (i, value) in row.iter().enumerate() {
                    if i > 0 {
                        line.push(sep);
                    }
                    csv_field(&value.clone().tidy(), sep, &mut line);
                }
                line.push('\n');
                self.out.write_all(line.as_bytes())?;
            }
            Format::Json | Format::Ndjson => {
                let mut object = Map::new();
                for (name, value) in self.columns.iter().zip(row) {
                    object.insert(name.clone(), value.clone().tidy().to_json());
                }
                let text = serde_json::to_string(&Json::Object(object)).map_err(io::Error::other)?;
                if self.format == Format::Json {
                    self.out.write_all(if self.rows == 0 { b"\n " } else { b",\n " })?;
                }
                self.out.write_all(text.as_bytes())?;
                if self.format == Format::Ndjson {
                    self.out.write_all(b"\n")?;
                }
            }
        }
        self.rows += 1;
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<()> {
        if self.format == Format::Json {
            self.out.write_all(if self.rows == 0 { b"]\n" } else { b"\n]\n" })?;
        }
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_like_polars() {
        assert_eq!(round3(0.0005), 0.0);
        assert_eq!(round3(0.0015), 0.002);
        assert_eq!(round3(1.0005), 1.0);
        assert_eq!(round3(2.0005), 2.001);
        assert_eq!(round3(0.1235), 0.124);
    }

    #[test]
    fn floats_are_written_like_polars() {
        assert_eq!(format_float(3.0), "3.0");
        assert_eq!(format_float(0.308), "0.308");
        assert_eq!(format_float(1e-7), "1e-7");
        assert_eq!(format_float(1e21), "1e+21");
    }

    #[test]
    fn csv_quoting_matches_polars() {
        let mut t = Table::new(&["s", "f", "i"]);
        t.rows.push(vec!["abc".into(), 3.0.into(), 1.into()]);
        t.rows.push(vec!["".into(), 0.308.into(), 2.into()]);
        t.rows.push(vec![Value::Null, Value::Float(f64::NAN), Value::Null]);
        t.rows.push(vec!["he said \"hi\"".into(), 1e21.into(), 4.into()]);
        t.rows.push(vec!["a,b".into(), 12345678.5.into(), 5.into()]);
        t.rows.push(vec!["עין".into(), (-2.5).into(), 6.into()]);
        let text = String::from_utf8(t.csv_bytes()).unwrap();
        assert_eq!(text, "\u{feff}s,f,i\nabc,3.0,1\n\"\",0.308,2\n,,\n\"he said \"\"hi\"\"\",1e+21,4\n\"a,b\",12345678.5,5\nעין,-2.5,6\n");
    }

    #[test]
    fn json_output_is_an_array_of_objects() {
        let mut out = Vec::new();
        let cols = vec!["a".to_string(), "b".to_string()];
        let mut w = RowWriter::new(&mut out, Format::Json, &cols).unwrap();
        w.write_row(&[1.into(), "x".into()]).unwrap();
        w.write_row(&[2.into(), Value::Null]).unwrap();
        w.finish().unwrap();
        let parsed: Json = serde_json::from_slice(&out).unwrap();
        assert_eq!(parsed[1]["a"], 2);
        assert!(parsed[1]["b"].is_null());
    }
}
