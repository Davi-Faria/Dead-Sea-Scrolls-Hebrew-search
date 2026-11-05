//! `dss-search stats ...`: the analyses of the Statistics tab from the command line.
//!
//! Every analysis takes the same filters as the Search tab (--corpus, --scrolls, --lang, --rec)
//! and counts either lexemes (--unit lexeme, the default) or words as written (--unit form).
//! The one-line description of each result goes to the error stream, so piping stays clean.

use crate::assets::Sources;
use clap::{Args, Subcommand};
use dss_search::builder::{self, DEFAULT_REF, DEFAULT_VERSION};
use dss_search::corpus::Corpus;
use dss_search::stats::{parse_unit, Answer, Scope};
use dss_search::table::{format_float, Format, RowWriter, Table, Value};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

#[derive(Args)]
pub struct Common {
    /// Count lexemes (all forms of a word together) or words as written
    #[arg(long, default_value = "lexeme", value_parser = ["lexeme", "form"])]
    unit: String,
    /// Non-biblical scrolls (default), biblical scrolls, or all
    #[arg(long, default_value = "nonbib", value_parser = ["nonbib", "bib", "all"])]
    corpus: String,
    /// Limit to some scrolls, e.g. "1QS, CD, 4Q4*"
    #[arg(long, default_value = "")]
    scrolls: String,
    /// Hebrew, Aramaic or both
    #[arg(long, default_value = "any", value_parser = ["any", "h", "a"])]
    lang: String,
    /// Leave out fully reconstructed words (skipfull) or any word with reconstructed letters (skipany)
    #[arg(long, default_value = "include", value_parser = ["include", "skipfull", "skipany"])]
    rec: String,
    /// How many rows to show (0 for all)
    #[arg(long)]
    top: Option<usize>,
    /// How to print the result
    #[arg(long, default_value = "table", value_parser = ["table", "csv", "json"])]
    format: String,
    /// Save the table to this file instead (.csv .tsv .json .ndjson)
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum StatsCommand {
    /// Describe the corpus
    Info(Common),
    /// The most frequent words
    Freq {
        #[command(flatten)]
        common: Common,
        /// Leave out words that occur fewer times
        #[arg(long, default_value_t = 1)]
        min_count: u64,
        /// Hide these words, e.g. "כל אשר את"
        #[arg(long, default_value = "")]
        exclude: String,
    },
    /// Where one word occurs, per scroll or cave
    Dist {
        /// A Hebrew word (a lexeme, or a spelling with --unit form)
        word: String,
        #[command(flatten)]
        common: Common,
        #[arg(long, default_value = "scroll", value_parser = ["scroll", "cave"])]
        by: String,
        /// Most occurrences first, or scroll order
        #[arg(long, default_value = "count", value_parser = ["count", "order"])]
        sort: String,
    },
    /// Words that occur near a word
    Colloc {
        /// A Hebrew word (a lexeme, or a spelling with --unit form)
        word: String,
        #[command(flatten)]
        common: Common,
        /// Words on each side (1 to 30)
        #[arg(long, default_value_t = 4)]
        window: usize,
        /// Leave out words that are near it fewer times
        #[arg(long, default_value_t = 3)]
        min_count: u64,
        /// Rank by log-likelihood, logDice or count
        #[arg(long, default_value = "ll", value_parser = ["ll", "logdice", "count"])]
        sort: String,
        /// Hide these words
        #[arg(long, default_value = "")]
        exclude: String,
    },
    /// Words typical of some scrolls compared with others
    Keyness {
        #[command(flatten)]
        common: Common,
        /// The scrolls to describe, e.g. "1QS" or "4Q26*"
        #[arg(long)]
        target: String,
        /// Compare with these scrolls (default: all other scrolls)
        #[arg(long, default_value = "")]
        reference: String,
        /// Words the target uses more (over), less (under) or both
        #[arg(long, default_value = "over", value_parser = ["over", "under", "both"])]
        direction: String,
        /// Leave out words that occur fewer times in all
        #[arg(long, default_value_t = 5)]
        min_count: u64,
        /// Rank by log-likelihood or size of the difference
        #[arg(long, default_value = "ll", value_parser = ["ll", "log_ratio"])]
        sort: String,
        /// Hide these words
        #[arg(long, default_value = "")]
        exclude: String,
    },
    /// Save a whole corpus table (words, lines, lemmas or scrolls)
    Export {
        #[arg(value_parser = ["words", "lines", "lemmas", "scrolls"])]
        table: String,
        /// The file to write (.csv .tsv .json .ndjson)
        #[arg(long, value_name = "FILE")]
        out: PathBuf,
    },
}

fn scope_of(common: &Common) -> Result<Scope, String> {
    Scope::new(&common.corpus, &common.scrolls, &common.lang, &common.rec).map_err(|e| e.0)
}

/// The format a file name asks for.
fn format_for(path: &std::path::Path) -> Result<Format, String> {
    let name = path.to_string_lossy();
    if name.to_lowercase().ends_with(".parquet") {
        return Err(
            "Parquet files are not supported by this program. Use .csv (Excel and most tools open it), .tsv, .json or .ndjson.".to_string()
        );
    }
    Format::from_extension(&name).ok_or_else(|| "Name the output file with .csv, .tsv, .json or .ndjson.".to_string())
}

fn write_table_file(table: &Table, path: &std::path::Path) -> Result<(), String> {
    let format = format_for(path)?;
    let file = File::create(path).map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    let mut writer = RowWriter::new(BufWriter::new(file), format, &table.columns).map_err(|e| e.to_string())?;
    for row in &table.rows {
        writer.write_row(row).map_err(|e| e.to_string())?;
    }
    writer.finish().map_err(|e| e.to_string())
}

fn cell_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(x) => format_float(*x),
        Value::Str(s) => s.clone(),
    }
}

/// Print a table with aligned columns (numbers to the right).
fn print_table(table: &Table, out: &mut impl Write) -> io::Result<()> {
    let table = table.tidy();
    let cells: Vec<Vec<String>> = table.rows.iter().map(|r| r.iter().map(cell_text).collect()).collect();
    let numeric: Vec<bool> = (0..table.columns.len())
        .map(|c| {
            table.rows.iter().all(|r| matches!(r[c], Value::Int(_) | Value::Float(_) | Value::Null))
                && table.rows.iter().any(|r| !matches!(r[c], Value::Null))
        })
        .collect();
    let widths: Vec<usize> = (0..table.columns.len())
        .map(|c| cells.iter().map(|r| r[c].chars().count()).chain([table.columns[c].chars().count()]).max().unwrap_or(0))
        .collect();
    let line = |texts: Vec<&str>| -> String {
        texts
            .iter()
            .enumerate()
            .map(|(c, t)| {
                let pad = widths[c].saturating_sub(t.chars().count());
                if numeric[c] {
                    format!("{}{}", " ".repeat(pad), t)
                } else {
                    format!("{}{}", t, " ".repeat(pad))
                }
            })
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    writeln!(out, "{}", line(table.columns.iter().map(|c| c.as_str()).collect()))?;
    writeln!(out, "{}", widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>().join("  "))?;
    for row in &cells {
        writeln!(out, "{}", line(row.iter().map(|c| c.as_str()).collect()))?;
    }
    Ok(())
}

/// Print or save a table, with its one-line description on the error stream.
fn show(table: &Table, text: &str, common: &Common) -> Result<(), String> {
    if let Some(path) = &common.out {
        write_table_file(table, path)?;
        eprintln!("{text} Saved to {}.", path.display());
        return Ok(());
    }
    eprintln!("{text}");
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let result = match common.format.as_str() {
        "csv" | "json" => {
            let format = if common.format == "csv" { Format::Csv } else { Format::Json };
            RowWriter::with_bom(&mut out, format, &table.columns, false).and_then(|mut w| {
                for row in &table.rows {
                    w.write_row(row)?;
                }
                w.finish()
            })
        }
        _ => print_table(table, &mut out).and_then(|_| out.flush()),
    };
    match result {
        // a closed pipe (for example `| head`) is not an error
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.map_err(|e| e.to_string()),
    }
}

fn show_answer(answer: &Answer, common: &Common) -> Result<(), String> {
    let mut text = answer.text().to_string();
    if let Some(list) = answer.summary.get("suggestions").and_then(|s| s.as_array()) {
        if !list.is_empty() {
            let words: Vec<&str> = list.iter().filter_map(|w| w.as_str()).collect();
            text += &format!(" Did you mean: {}?", words.join(", "));
        }
    }
    show(&answer.table, &text, common)
}

pub fn run_stats(sources: &Sources, command: StatsCommand) -> Result<(), String> {
    let corpus: Corpus = sources.load_corpus()?;
    let fail = |e: dss_search::stats::StatsError| e.0;
    match command {
        StatsCommand::Info(common) => {
            let mut table = Table::new(&["what", "value"]);
            for (what, value) in corpus.info() {
                table.rows.push(vec![what.into(), value.into()]);
            }
            show(&table, "Corpus summary.", &common)
        }
        StatsCommand::Export { table, out } => {
            let format = format_for(&out)?;
            let columns = Corpus::frame_columns(&table)?;
            let file = File::create(&out).map_err(|e| format!("Could not create {}: {e}", out.display()))?;
            let mut writer = RowWriter::new(BufWriter::new(file), format, &columns).map_err(|e| e.to_string())?;
            corpus.write_frame(&table, &mut writer).map_err(|e| e.to_string())?;
            writer.finish().map_err(|e| e.to_string())?;
            eprintln!("{table}: {} rows. Saved to {}.", dss_search::stats::fmt_int(corpus.frame_len(&table)), out.display());
            Ok(())
        }
        StatsCommand::Freq { common, min_count, exclude } => {
            let answer = corpus
                .frequencies(&scope_of(&common)?, parse_unit(&common.unit).map_err(fail)?, min_count, common.top.unwrap_or(50), &exclude)
                .map_err(fail)?;
            show_answer(&answer, &common)
        }
        StatsCommand::Dist { word, common, by, sort } => {
            let answer = corpus
                .distribution(&word, &scope_of(&common)?, parse_unit(&common.unit).map_err(fail)?, &by, &sort, common.top.unwrap_or(0))
                .map_err(fail)?;
            show_answer(&answer, &common)
        }
        StatsCommand::Colloc { word, common, window, min_count, sort, exclude } => {
            let answer = corpus
                .collocations(
                    &word,
                    &scope_of(&common)?,
                    parse_unit(&common.unit).map_err(fail)?,
                    window,
                    min_count,
                    &sort,
                    common.top.unwrap_or(50),
                    &exclude,
                    true,
                )
                .map_err(fail)?;
            show_answer(&answer, &common)
        }
        StatsCommand::Keyness { common, target, reference, direction, min_count, sort, exclude } => {
            let answer = corpus
                .keyness(
                    &target,
                    &reference,
                    &scope_of(&common)?,
                    parse_unit(&common.unit).map_err(fail)?,
                    min_count,
                    &sort,
                    &direction,
                    common.top.unwrap_or(50),
                    &exclude,
                )
                .map_err(fail)?;
            show_answer(&answer, &common)
        }
    }
}

// --------------------------------------------------------------------------
// build: rebuild the data
// --------------------------------------------------------------------------

#[derive(Args)]
pub struct BuildArgs {
    /// Folder with the .tf files (for example a local clone of github.com/ETCBC/dss: tf/2.0.1)
    #[arg(long, value_name = "DIR")]
    tf_dir: Option<PathBuf>,
    /// Git commit or branch of ETCBC/dss to download
    #[arg(long = "ref", default_value = DEFAULT_REF)]
    reference: String,
    /// Dataset version folder inside tf/
    #[arg(long, default_value = DEFAULT_VERSION)]
    version: String,
    /// Where downloaded .tf files are kept
    #[arg(long, default_value = ".cache/dss-tf")]
    cache: PathBuf,
    /// Output file (bible.json is written next to it)
    #[arg(long, default_value = "data/dss.json")]
    out: PathBuf,
}

#[cfg(feature = "download")]
fn fetch(args: &BuildArgs) -> Result<PathBuf, String> {
    crate::download::download_features(&args.cache, &args.reference, &args.version)
}

#[cfg(not(feature = "download"))]
fn fetch(_args: &BuildArgs) -> Result<PathBuf, String> {
    Err("This copy of the program does not download anything. Either get the dataset yourself (git clone --depth 1 https://github.com/ETCBC/dss) \
         and pass the folder with --tf-dir (for example --tf-dir dss/tf/2.0.1), or build the program with `cargo build --release --features download`."
        .to_string())
}

pub fn run_build(args: BuildArgs) -> Result<(), String> {
    let (tf_dir, label) = match &args.tf_dir {
        Some(dir) => (dir.clone(), "local".to_string()),
        None => (fetch(&args)?, args.reference.clone()),
    };
    builder::build_and_write(&tf_dir, &label, &args.out, &|message| println!("{message}"))
}
