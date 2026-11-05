//! The query interface used by the server: takes the strings of a URL query.
//!
//! What each analysis does when a setting is not given. The Statistics tab (search.js) uses the
//! same values, and `tests/page_agrees.rs` checks that the two agree.

use crate::corpus::Corpus;
use crate::stats::{check_analysis, parse_unit, Answer, Scope, StatsError, StatsResult};
use std::collections::HashMap;

pub const COMMON_DEFAULTS: [(&str, &str); 4] = [("unit", "lexeme"), ("corpus", "nonbib"), ("lang", "any"), ("rec", "include")];

/// (analysis, setting, default) for the numbers and choices that differ per analysis.
pub const DEFAULTS: [(&str, &str, &str); 13] = [
    ("freq", "min", "1"),
    ("freq", "top", "100"),
    ("dist", "top", "0"),
    ("dist", "by", "scroll"),
    ("dist", "sort", "count"),
    ("colloc", "min", "3"),
    ("colloc", "top", "50"),
    ("colloc", "window", "4"),
    ("colloc", "sort", "ll"),
    ("keyness", "min", "5"),
    ("keyness", "top", "50"),
    ("keyness", "sort", "ll"),
    ("keyness", "direction", "over"),
];

/// The value an analysis uses for a setting that was not given (empty when it has none).
pub fn default_for(analysis: &str, name: &str) -> &'static str {
    DEFAULTS.iter().find(|(a, n, _)| *a == analysis && *n == name).map(|(_, _, d)| *d).unwrap_or("")
}

fn text<'a>(params: &'a HashMap<String, String>, name: &str, default: &'a str) -> &'a str {
    params.get(name).map(|s| s.as_str()).unwrap_or(default)
}

fn number(params: &HashMap<String, String>, name: &str, default: i64, low: i64, high: i64) -> StatsResult<i64> {
    let raw = match params.get(name) {
        None => return Ok(default),
        Some(raw) if raw.is_empty() => return Ok(default),
        Some(raw) => raw,
    };
    let value: i64 = raw.trim().parse().map_err(|_| StatsError(format!("{name} must be a whole number.")))?;
    if !(low..=high).contains(&value) {
        return Err(StatsError(format!("{name} must be between {low} and {high}.")));
    }
    Ok(value)
}

/// Run one analysis from URL-style parameters.
///
/// Parameters (all optional unless noted):
/// * common: unit, corpus, scrolls, lang, rec, top
/// * freq: min, exclude
/// * dist: word (required), by, sort
/// * colloc: word (required), window, min, sort, exclude
/// * keyness: target (required), reference, direction, min, sort, exclude
///
/// Returns an error with a message fit to show to a person when something is not valid.
pub fn query(corpus: &Corpus, analysis: &str, params: &HashMap<String, String>) -> StatsResult<Answer> {
    check_analysis(analysis)?;
    let d = |name: &str| default_for(analysis, name);
    let unit_name = text(params, "unit", COMMON_DEFAULTS[0].1);
    let scope = Scope::new(
        text(params, "corpus", COMMON_DEFAULTS[1].1),
        text(params, "scrolls", ""),
        text(params, "lang", COMMON_DEFAULTS[2].1),
        text(params, "rec", COMMON_DEFAULTS[3].1),
    )?;
    let top = number(params, "top", d("top").parse().unwrap_or(0), 0, 5000)? as usize;
    let exclude = text(params, "exclude", "");
    let min = |params: &HashMap<String, String>| number(params, "min", d("min").parse().unwrap_or(1), 1, 10_000_000);

    match analysis {
        "freq" => {
            let min_count = min(params)? as u64;
            corpus.frequencies(&scope, parse_unit(unit_name)?, min_count, top, exclude)
        }
        "dist" => {
            let (by, sort) = (text(params, "by", d("by")), text(params, "sort", d("sort")));
            corpus.distribution(text(params, "word", ""), &scope, parse_unit(unit_name)?, by, sort, top)
        }
        "colloc" => {
            let window = number(params, "window", d("window").parse().unwrap_or(4), 1, 30)? as usize;
            let min_count = min(params)? as u64;
            corpus.collocations(
                text(params, "word", ""),
                &scope,
                parse_unit(unit_name)?,
                window,
                min_count,
                text(params, "sort", d("sort")),
                top,
                exclude,
                true,
            )
        }
        _ => {
            let min_count = min(params)? as u64;
            corpus.keyness(
                text(params, "target", ""),
                text(params, "reference", ""),
                &scope,
                parse_unit(unit_name)?,
                min_count,
                text(params, "sort", d("sort")),
                text(params, "direction", d("direction")),
                top,
                exclude,
            )
        }
    }
}

/// Split a URL query string (`a=1&b=%D7%90`) into settings. The first value of a repeated name
/// wins, blank values are kept, `+` means a space, and bad UTF-8 is replaced.
pub fn parse_query(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        out.entry(percent_decode(name)).or_insert_with(|| percent_decode(value));
    }
    out
}

pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => match (bytes.get(i + 1).and_then(|b| hex(*b)), bytes.get(i + 2).and_then(|b| hex(*b))) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_queries() {
        let q = parse_query("word=%D7%99%D7%97%D7%93&top=&scrolls=1QS%2C+CD&word=ignored");
        assert_eq!(q["word"], "יחד");
        assert_eq!(q["top"], "");
        assert_eq!(q["scrolls"], "1QS, CD");
    }

    #[test]
    fn tolerates_broken_escapes() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%41"), "A");
    }
}
