//! Statistics on the Dead Sea Scrolls text: the four analyses of the Statistics tab.
//!
//! This is a port of `dss_stats.py` (which used Polars) with the same numbers, columns,
//! sorting and messages. See `corpus.rs` for how words are counted.

use crate::corpus::*;
use crate::hebrew::normalize_hebrew;
use crate::table::{Table, Value};
use serde_json::{json, Map, Value as Json};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;

pub use crate::corpus::Unit;

pub const UNITS: [&str; 2] = ["lexeme", "form"];
pub const CORPORA: [&str; 3] = ["nonbib", "bib", "all"];
pub const LANGS: [&str; 3] = ["any", "h", "a"];
pub const RECS: [&str; 3] = ["include", "skipfull", "skipany"];
pub const ANALYSES: [&str; 4] = ["freq", "dist", "colloc", "keyness"];
pub const DIST_BY: [&str; 2] = ["scroll", "cave"];
pub const DIST_SORTS: [&str; 2] = ["count", "order"];
pub const COLLOC_SORTS: [&str; 3] = ["ll", "logdice", "count"];
pub const KEYNESS_SORTS: [&str; 2] = ["ll", "log_ratio"];
pub const DIRECTIONS: [&str; 3] = ["over", "under", "both"];

// --------------------------------------------------------------------------
// Errors, small helpers
// --------------------------------------------------------------------------

/// A problem with the question asked; the message is fit to show to a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsError(pub String);

impl fmt::Display for StatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for StatsError {}

pub type StatsResult<T> = Result<T, StatsError>;

fn fail<T>(message: impl Into<String>) -> StatsResult<T> {
    Err(StatsError(message.into()))
}

pub fn check_analysis(analysis: &str) -> StatsResult<()> {
    check("analysis", analysis, &ANALYSES)
}

fn check(name: &str, value: &str, allowed: &[&str]) -> StatsResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        fail(format!("{name} must be one of {} (not '{value}').", allowed.join(", ")))
    }
}

/// An integer with thousands separators, like Python's `{:,}`.
pub fn fmt_int(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Words to hide, as normalised letters, from a string like "כל אשר" (separated by spaces, commas or semicolons).
fn exclusion(exclude: &str) -> HashSet<String> {
    exclude.split(|c: char| c.is_whitespace() || c == ',' || c == ';').map(normalize_hebrew).filter(|n| !n.is_empty()).collect()
}

/// Order floats, with NaN last (as the largest value), so that sorting never panics.
fn cmp_f64(a: f64, b: f64) -> Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
    }
}

/// Log-likelihood (G2) of a 2x2 table: o11 = occurrences in the first group, r1 = size of the
/// first group, c1 = occurrences of the word in both groups, n = size of both groups together.
/// Cells that are zero contribute nothing.
pub fn g2(o11: f64, r1: f64, c1: f64, n: f64) -> f64 {
    let o12 = r1 - o11;
    let o21 = c1 - o11;
    let o22 = n - r1 - c1 + o11;
    let term = |o: f64, row: f64, col: f64| {
        let expected = div_const(row * col, n);
        if o > 0.0 {
            o * (o / expected).ln()
        } else {
            0.0
        }
    };
    2.0 * (term(o11, r1, c1) + term(o12, r1, n - c1) + term(o21, n - r1, c1) + term(o22, n - r1, n - c1))
}

/// Divide by a single number the way Polars does: by multiplying with its reciprocal. This
/// differs from true division in the last binary digit, which is enough to change the order of
/// rows that are tied in theory (for example 2 of 4 and 3 of 6), so it is done the same way here
/// to give the same tables as the original version. Dividing one column by another is exact.
#[inline]
fn div_const(x: f64, divisor: f64) -> f64 {
    x * (1.0 / divisor)
}

fn log2(x: f64) -> f64 {
    x.ln() / std::f64::consts::LN_2
}

// --------------------------------------------------------------------------
// Scope and answer
// --------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorpusSel {
    NonBib,
    Bib,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LangSel {
    Any,
    Hebrew,
    Aramaic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecSel {
    Include,
    SkipFull,
    SkipAny,
}

/// Which words take part in an analysis. The options match the Search tab's.
#[derive(Clone, Debug)]
pub struct Scope {
    pub corpus: CorpusSel,
    /// "1QS, CD, 4Q4*"; empty means every scroll
    pub scrolls: String,
    pub lang: LangSel,
    pub rec: RecSel,
}

impl Default for Scope {
    fn default() -> Self {
        Scope { corpus: CorpusSel::NonBib, scrolls: String::new(), lang: LangSel::Any, rec: RecSel::Include }
    }
}

impl Scope {
    /// From the names used in URLs and on the command line (nonbib/bib/all, any/h/a, include/skipfull/skipany).
    pub fn new(corpus: &str, scrolls: &str, lang: &str, rec: &str) -> StatsResult<Scope> {
        check("corpus", corpus, &CORPORA)?;
        check("lang", lang, &LANGS)?;
        check("rec", rec, &RECS)?;
        Ok(Scope {
            corpus: match corpus {
                "bib" => CorpusSel::Bib,
                "all" => CorpusSel::All,
                _ => CorpusSel::NonBib,
            },
            scrolls: scrolls.to_string(),
            lang: match lang {
                "h" => LangSel::Hebrew,
                "a" => LangSel::Aramaic,
                _ => LangSel::Any,
            },
            rec: match rec {
                "skipfull" => RecSel::SkipFull,
                "skipany" => RecSel::SkipAny,
                _ => RecSel::Include,
            },
        })
    }
}

pub fn parse_unit(unit: &str) -> StatsResult<Unit> {
    check("unit", unit, &UNITS)?;
    Ok(if unit == "form" { Unit::Form } else { Unit::Lexeme })
}

/// A table and a summary. `summary["text"]` is a one-line description for people.
#[derive(Clone, Debug)]
pub struct Answer {
    pub table: Table,
    pub summary: Map<String, Json>,
}

impl Answer {
    /// `{"columns", "rows", "summary"}`; numbers are rounded to 3 decimals.
    pub fn to_json(&self) -> Json {
        let (columns, rows) = self.table.to_json_parts();
        let mut object = Map::new();
        object.insert("columns".into(), columns);
        object.insert("rows".into(), rows);
        object.insert("summary".into(), Json::Object(self.summary.clone()));
        Json::Object(object)
    }

    pub fn text(&self) -> &str {
        self.summary.get("text").and_then(|t| t.as_str()).unwrap_or("")
    }
}

fn summary(pairs: Vec<(&str, Json)>) -> Map<String, Json> {
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), v);
    }
    map
}

/// A scope prepared for fast checks of single tokens.
struct Prepared {
    corpus: CorpusSel,
    chosen: Option<Vec<bool>>,
    lang: LangSel,
    rec_mask: u8,
}

#[derive(Clone, Copy)]
struct Elig {
    scroll: u32,
    k: u32,
    bits: u8,
}

impl Prepared {
    /// The word's key if the token takes part in the analysis, else None.
    #[inline]
    fn accepts(&self, c: &Corpus, unit: Unit, t: &Token) -> Option<u32> {
        let k = c.key(unit, t)?;
        match self.corpus {
            CorpusSel::NonBib if t.biblical => return None,
            CorpusSel::Bib if !t.biblical => return None,
            _ => {}
        }
        if let Some(chosen) = &self.chosen {
            if !chosen[t.scroll as usize] {
                return None;
            }
        }
        match self.lang {
            LangSel::Aramaic if !c.is_aramaic(t) => return None,
            LangSel::Hebrew if c.is_aramaic(t) => return None,
            _ => {}
        }
        if self.rec_mask != 0 && t.bits & self.rec_mask != 0 {
            return None;
        }
        Some(k)
    }
}

/// How many rows to keep: 0 means all of them.
fn limit(top: usize) -> usize {
    if top == 0 {
        usize::MAX
    } else {
        top
    }
}

fn lang_value(label: &Label) -> Value {
    label.lang.clone().into()
}

impl Corpus {
    fn prepare(&self, scope: &Scope) -> Prepared {
        let ids: Vec<String> = self.scrolls.iter().map(|s| s.id.clone()).collect();
        let chosen = select_scrolls(&scope.scrolls, &ids).map(|list| {
            let mut mask = vec![false; ids.len()];
            for i in list {
                mask[i] = true;
            }
            mask
        });
        Prepared {
            corpus: scope.corpus,
            chosen,
            lang: scope.lang,
            rec_mask: match scope.rec {
                RecSel::SkipAny => BIT_REC_ANY,
                RecSel::SkipFull => BIT_REC_FULL,
                RecSel::Include => 0,
            },
        }
    }

    /// The tokens that take part: scroll, key (the word's id for this unit) and status bits.
    fn eligible(&self, scope: &Scope, unit: Unit) -> Vec<Elig> {
        let p = self.prepare(scope);
        self.tokens.iter().filter_map(|t| p.accepts(self, unit, t).map(|k| Elig { scroll: t.scroll, k, bits: t.bits })).collect()
    }

    /// Words that start with (or else contain) the typed letters, most frequent first.
    pub fn suggest(&self, query: &str, unit: Unit, limit: usize) -> Vec<String> {
        let q = normalize_hebrew(query);
        if q.is_empty() {
            return Vec::new();
        }
        let labels: Vec<&Label> = self.labels(unit).iter().filter(|l| l.total > 0).collect();
        let by_frequency = |a: &&Label, b: &&Label| b.total.cmp(&a.total).then_with(|| a.item.cmp(&b.item));
        let mut starts: Vec<&Label> = labels.iter().copied().filter(|l| l.item_norm.starts_with(&q)).collect();
        let mut contains: Vec<&Label> =
            labels.iter().copied().filter(|l| l.item_norm.contains(&q) && !l.item_norm.starts_with(&q)).collect();
        starts.sort_by(by_frequency);
        contains.sort_by(by_frequency);
        let mut seen = HashSet::new();
        starts.into_iter().chain(contains).filter(|l| seen.insert(l.item.clone())).map(|l| l.item.clone()).take(limit).collect()
    }

    /// The words (keys) that a typed word names.
    fn resolve(&self, word: &str, unit: Unit) -> StatsResult<Vec<u32>> {
        let text = word.trim();
        if text.is_empty() {
            return fail("Enter a Hebrew word.");
        }
        if text.chars().any(char::is_whitespace) {
            return fail("Give one word at a time.");
        }
        let q = normalize_hebrew(text);
        if q.is_empty() {
            return fail("Enter a Hebrew word.");
        }
        Ok(self.labels(unit).iter().enumerate().filter(|(_, l)| l.total > 0 && l.item_norm == q).map(|(k, _)| k as u32).collect())
    }

    /// The text to show for the word(s) a typed word names, without repeats.
    fn label_text(&self, ks: &[u32], unit: Unit) -> String {
        let labels = self.labels(unit);
        let mut seen: Vec<&str> = Vec::new();
        for &k in ks {
            let item = labels[k as usize].item.as_str();
            if !seen.contains(&item) {
                seen.push(item);
            }
        }
        seen.join(" / ")
    }

    // ---- frequencies --------------------------------------------------------

    /// The most frequent words: count, per million words, scrolls, spread (DP), reconstructed.
    ///
    /// DP (deviation of proportions, Gries 2008) measures how evenly a word is spread over the
    /// scrolls compared with how large they are: near 0 means evenly, near 1 means in few scrolls.
    /// `exclude` hides words from the list (they still count towards the totals). `top` = 0 means all.
    pub fn frequencies(&self, scope: &Scope, unit: Unit, min_count: u64, top: usize, exclude: &str) -> StatsResult<Answer> {
        let elig = self.eligible(scope, unit);
        let n_total = elig.len();
        let nk = self.key_count(unit);

        let mut count = vec![0u32; nk];
        let mut reconstructed = vec![0u32; nk];
        let mut sizes = vec![0u32; self.scrolls.len()];
        for e in &elig {
            count[e.k as usize] += 1;
            if e.bits & BIT_REC_ANY != 0 {
                reconstructed[e.k as usize] += 1;
            }
            sizes[e.scroll as usize] += 1;
        }
        let n_scrolls = sizes.iter().filter(|&&s| s > 0).count();

        // spread over the scrolls
        let mut pairs: Vec<(u32, u32)> = elig.iter().map(|e| (e.k, e.scroll)).collect();
        pairs.sort_unstable();
        let denominator = n_total.max(1) as f64;
        let mut spread: Vec<(u32, f64)> = vec![(0, 0.0); nk];
        let mut i = 0;
        while i < pairs.len() {
            let k = pairs[i].0;
            let n_k = count[k as usize] as f64;
            let (mut deviation, mut expected_sum, mut in_scrolls) = (0.0f64, 0.0f64, 0u32);
            while i < pairs.len() && pairs[i].0 == k {
                let scroll = pairs[i].1;
                let mut c = 0u32;
                while i < pairs.len() && pairs[i] == (k, scroll) {
                    c += 1;
                    i += 1;
                }
                let observed = c as f64 / n_k;
                let expected = div_const(sizes[scroll as usize] as f64, denominator);
                deviation += (observed - expected).abs();
                expected_sum += expected;
                in_scrolls += 1;
            }
            spread[k as usize] = (in_scrolls, (0.5 * (deviation + (1.0 - expected_sum))).clamp(0.0, 1.0));
        }

        let labels = self.labels(unit);
        let mut rows: Vec<u32> = (0..nk as u32).filter(|&k| count[k as usize] > 0).collect();
        let distinct = rows.len();
        rows.retain(|&k| count[k as usize] as u64 >= min_count);
        let mut hidden = 0;
        let skip = exclusion(exclude);
        if !skip.is_empty() {
            let before = rows.len();
            rows.retain(|&k| !skip.contains(&labels[k as usize].item_norm));
            hidden = before - rows.len();
        }
        rows.sort_by(|&a, &b| {
            count[b as usize].cmp(&count[a as usize]).then_with(|| labels[a as usize].item.cmp(&labels[b as usize].item)).then(a.cmp(&b))
        });
        let matching = rows.len();
        rows.truncate(limit(top));

        let with_lang = unit == Unit::Lexeme;
        let mut columns = vec!["rank", "item"];
        if with_lang {
            columns.push("lang");
        }
        columns.extend(["count", "per_million", "scrolls", "dp", "reconstructed"]);
        let mut table = Table::new(&columns);
        for (r, &k) in rows.iter().enumerate() {
            let label = &labels[k as usize];
            let mut row: Vec<Value> = vec![(r + 1).into(), label.item.as_str().into()];
            if with_lang {
                row.push(lang_value(label));
            }
            row.push(count[k as usize].into());
            row.push((div_const(count[k as usize] as f64, denominator) * 1e6).into());
            row.push(spread[k as usize].0.into());
            row.push(spread[k as usize].1.into());
            row.push(reconstructed[k as usize].into());
            table.rows.push(row);
        }

        let noun = if unit == Unit::Lexeme { "lexemes" } else { "different spellings" };
        let mut text = format!("{} words counted in {} scrolls; {} {}", fmt_int(n_total), fmt_int(n_scrolls), fmt_int(distinct), noun);
        if min_count > 1 {
            text += &format!(", {} with at least {} occurrences", fmt_int(matching), min_count);
        }
        text.push('.');
        let shown = table.len();
        Ok(Answer {
            summary: summary(vec![
                ("text", text.into()),
                ("tokens", n_total.into()),
                ("scrolls", n_scrolls.into()),
                ("distinct", distinct.into()),
                ("matching", matching.into()),
                ("shown", shown.into()),
                ("hidden", hidden.into()),
                ("unit", unit.as_str().into()),
            ]),
            table,
        })
    }

    // ---- distribution -------------------------------------------------------

    /// Where one word occurs: per scroll (or per cave), with counts, per million and share of hits.
    pub fn distribution(&self, word: &str, scope: &Scope, unit: Unit, by: &str, sort: &str, top: usize) -> StatsResult<Answer> {
        check("by", by, &DIST_BY)?;
        check("sort", sort, &DIST_SORTS)?;
        let ks = self.resolve(word, unit)?;
        let wanted: HashSet<u32> = ks.iter().copied().collect();
        let elig = self.eligible(scope, unit);

        let n_scrolls = self.scrolls.len();
        let mut sizes = vec![0u32; n_scrolls];
        let mut hits = vec![0u32; n_scrolls];
        let mut recon = vec![0u32; n_scrolls];
        for e in &elig {
            sizes[e.scroll as usize] += 1;
            if wanted.contains(&e.k) {
                hits[e.scroll as usize] += 1;
                if e.bits & BIT_REC_ANY != 0 {
                    recon[e.scroll as usize] += 1;
                }
            }
        }
        let scrolls_in_scope = sizes.iter().filter(|&&s| s > 0).count();
        let scrolls_with_hits = hits.iter().filter(|&&h| h > 0).count();
        let total: u64 = hits.iter().map(|&h| h as u64).sum();
        let share_of = |count: u64| div_const(count as f64, total.max(1) as f64) * 100.0;

        struct Row {
            order: usize,
            count: u64,
            values: Vec<Value>,
        }
        let mut rows: Vec<Row> = Vec::new();
        let columns: Vec<&str>;

        if by == "scroll" {
            columns = vec!["group", "name", "count", "words", "per_million", "share", "reconstructed"];
            let mut ordered: Vec<&str> = self.scrolls.iter().map(|s| s.id.as_str()).collect();
            ordered.sort_by_cached_key(|id| scroll_sort_key(id));
            ordered.dedup();
            let rank: std::collections::HashMap<&str, usize> = ordered.iter().enumerate().map(|(i, id)| (*id, i)).collect();
            for (i, s) in self.scrolls.iter().enumerate() {
                if hits[i] == 0 {
                    continue;
                }
                let (count, words) = (hits[i] as u64, sizes[i] as f64);
                rows.push(Row {
                    order: rank[s.id.as_str()],
                    count,
                    values: vec![
                        s.id.as_str().into(),
                        s.name.as_str().into(),
                        count.into(),
                        sizes[i].into(),
                        (count as f64 / words * 1e6).into(),
                        share_of(count).into(),
                        recon[i].into(),
                    ],
                });
            }
        } else {
            columns = vec!["group", "scrolls", "count", "words", "per_million", "share", "reconstructed"];
            let mut caves: Vec<&str> = self.scrolls.iter().map(|s| s.cave.as_str()).collect();
            caves.sort_by_cached_key(|id| scroll_sort_key(id));
            caves.dedup();
            for (order, cave) in caves.iter().enumerate() {
                let (mut words, mut count, mut reconstructed, mut with_hits) = (0u64, 0u64, 0u64, 0u64);
                for (i, s) in self.scrolls.iter().enumerate() {
                    if s.cave == *cave {
                        words += sizes[i] as u64;
                        count += hits[i] as u64;
                        reconstructed += recon[i] as u64;
                        with_hits += (hits[i] > 0) as u64;
                    }
                }
                if count == 0 {
                    continue;
                }
                rows.push(Row {
                    order,
                    count,
                    values: vec![
                        cave.to_string().into(),
                        with_hits.into(),
                        count.into(),
                        words.into(),
                        (count as f64 / words as f64 * 1e6).into(),
                        share_of(count).into(),
                        reconstructed.into(),
                    ],
                });
            }
        }

        if sort == "order" {
            rows.sort_by_key(|r| r.order);
        } else {
            rows.sort_by(|a, b| b.count.cmp(&a.count).then(a.order.cmp(&b.order)));
        }
        let n_groups = rows.len();
        rows.truncate(limit(top));
        let mut table = Table::new(&columns);
        table.rows = rows.into_iter().map(|r| r.values).collect();

        let label = self.label_text(&ks, unit);
        let text = if ks.is_empty() {
            format!("No {} is spelled like that.", if unit == Unit::Lexeme { "lexeme" } else { "word" })
        } else if total == 0 {
            format!("{label} does not occur in these scrolls under these settings.")
        } else {
            format!(
                "{label}: {} occurrences in {} of {} scrolls{}.",
                fmt_int(total as usize),
                fmt_int(scrolls_with_hits),
                fmt_int(scrolls_in_scope),
                if by == "cave" { format!(" (shown by cave: {} groups)", fmt_int(n_groups)) } else { String::new() }
            )
        };
        let mut s = summary(vec![
            ("text", text.into()),
            ("found", (!ks.is_empty()).into()),
            ("item", label.into()),
            ("total", total.into()),
            ("scrolls_with_hits", scrolls_with_hits.into()),
            ("scrolls_in_scope", scrolls_in_scope.into()),
            ("tokens", elig.len().into()),
            ("by", by.into()),
        ]);
        if ks.is_empty() {
            s.insert("suggestions".into(), json!(self.suggest(word, unit, 12)));
        }
        Ok(Answer { table, summary: s })
    }

    // ---- collocations -------------------------------------------------------

    /// Words that occur near a word more often than chance would suggest.
    ///
    /// For each occurrence of the word, the `window` words on each side count as "near", unless
    /// a gap or the end of the column or fragment comes first. A word that is near several
    /// occurrences is counted once, at its nearest one (left or right). Each candidate is scored
    /// on the 2x2 table "near the word / elsewhere" against "this word / other words":
    ///
    /// * `ll`: log-likelihood (G2); higher means a stronger association
    /// * `logdice`: 14 + log2(2 * near / (word frequency + candidate frequency)); at most 14,
    ///   and less swayed by very common words than `ll`
    ///
    /// `count` is how often the candidate is near the word, `left` and `right` split that by
    /// side, and `freq` is how often the candidate occurs in these scrolls altogether. With
    /// `attraction_only` a candidate must occur near the word more than expected.
    #[allow(clippy::too_many_arguments)]
    pub fn collocations(
        &self,
        word: &str,
        scope: &Scope,
        unit: Unit,
        window: usize,
        min_count: u64,
        sort: &str,
        top: usize,
        exclude: &str,
        attraction_only: bool,
    ) -> StatsResult<Answer> {
        check("sort", sort, &COLLOC_SORTS)?;
        if !(1..=30).contains(&window) {
            return fail("The window must be between 1 and 30 words.");
        }
        let ks = self.resolve(word, unit)?;
        let wanted: HashSet<u32> = ks.iter().copied().collect();
        let prepared = self.prepare(scope);

        // Only words take part in the sequence; distance counts every word, even those the
        // filters leave out, so that skipping reconstructed words does not make their
        // neighbours adjacent.
        let words: Vec<&Token> = self.tokens.iter().filter(|t| t.is_word()).collect();
        let keyed: Vec<Option<u32>> = words.iter().map(|t| prepared.accepts(self, unit, t)).collect();
        let n_total = keyed.iter().flatten().count();
        let nodes: Vec<usize> = (0..words.len()).filter(|&i| keyed[i].is_some_and(|k| wanted.contains(&k))).collect();
        let f_node = nodes.len();

        // For every word near a node: its distance and side from the nearest node (left if tied).
        let w = window as i64;
        let mut nearest: Vec<Option<(i64, i64)>> = vec![None; words.len()];
        for &i in &nodes {
            let seg = words[i].seg;
            for off in -w..=w {
                let j = i as i64 + off;
                if off == 0 || j < 0 || j as usize >= words.len() {
                    continue;
                }
                let j = j as usize;
                if words[j].seg != seg || keyed[j].is_none() {
                    continue;
                }
                let candidate = (off.abs(), off);
                if nearest[j].is_none_or(|best| candidate < best) {
                    nearest[j] = Some(candidate);
                }
            }
        }

        let nk = self.key_count(unit);
        let (mut near, mut left, mut right, mut freq) = (vec![0u32; nk], vec![0u32; nk], vec![0u32; nk], vec![0u32; nk]);
        let mut r1 = 0usize;
        for (j, key) in keyed.iter().enumerate() {
            if let Some(k) = key {
                freq[*k as usize] += 1;
                if let Some((_, off)) = nearest[j] {
                    r1 += 1;
                    near[*k as usize] += 1;
                    if off < 0 {
                        left[*k as usize] += 1;
                    } else {
                        right[*k as usize] += 1;
                    }
                }
            }
        }

        struct Row {
            k: u32,
            ll: f64,
            logdice: f64,
        }
        let n = n_total.max(1) as f64;
        let labels = self.labels(unit);
        let mut rows: Vec<Row> = Vec::new();
        for k in 0..nk {
            if near[k] == 0 {
                continue;
            }
            let o11 = near[k] as f64;
            let c1 = freq[k] as f64;
            let expected = div_const(r1 as f64 * c1, n);
            if attraction_only && o11 <= expected {
                continue;
            }
            if (near[k] as u64) < min_count {
                continue;
            }
            rows.push(Row { k: k as u32, ll: g2(o11, r1 as f64, c1, n), logdice: 14.0 + log2(2.0 * o11 / (f_node as f64 + c1)) });
        }
        let mut hidden = 0;
        let skip = exclusion(exclude);
        if !skip.is_empty() {
            let before = rows.len();
            rows.retain(|r| !skip.contains(&labels[r.k as usize].item_norm));
            hidden = before - rows.len();
        }
        rows.sort_by(|a, b| {
            let (ka, kb) = (a.k as usize, b.k as usize);
            let primary = match sort {
                "ll" => cmp_f64(b.ll, a.ll),
                "logdice" => cmp_f64(b.logdice, a.logdice),
                _ => near[kb].cmp(&near[ka]),
            };
            primary
                .then_with(|| if sort == "count" { Ordering::Equal } else { near[kb].cmp(&near[ka]) })
                .then_with(|| labels[ka].item.cmp(&labels[kb].item))
                .then(a.k.cmp(&b.k))
        });
        let matching = rows.len();
        rows.truncate(limit(top));

        let with_lang = unit == Unit::Lexeme;
        let mut columns = vec!["rank", "item"];
        if with_lang {
            columns.push("lang");
        }
        columns.extend(["count", "left", "right", "freq", "ll", "logdice"]);
        let mut table = Table::new(&columns);
        for (r, row) in rows.iter().enumerate() {
            let k = row.k as usize;
            let mut cells: Vec<Value> = vec![(r + 1).into(), labels[k].item.as_str().into()];
            if with_lang {
                cells.push(lang_value(&labels[k]));
            }
            cells.extend([near[k].into(), left[k].into(), right[k].into(), freq[k].into(), row.ll.into(), row.logdice.into()]);
            table.rows.push(cells);
        }

        let label = self.label_text(&ks, unit);
        let text = if ks.is_empty() {
            format!("No {} is spelled like that.", if unit == Unit::Lexeme { "lexeme" } else { "word" })
        } else if f_node == 0 {
            format!("{label} does not occur in these scrolls under these settings.")
        } else {
            format!(
                "{label} occurs {} times; {} words lie within {window} {} of it, in {} different {} listed below.",
                fmt_int(f_node),
                fmt_int(r1),
                if window == 1 { "word" } else { "words" },
                fmt_int(matching),
                if unit == Unit::Lexeme { "lexemes" } else { "spellings" }
            )
        };
        let shown = table.len();
        let mut s = summary(vec![
            ("text", text.into()),
            ("found", (!ks.is_empty()).into()),
            ("item", label.into()),
            ("occurrences", f_node.into()),
            ("near_words", r1.into()),
            ("tokens", n_total.into()),
            ("window", window.into()),
            ("matching", matching.into()),
            ("shown", shown.into()),
            ("hidden", hidden.into()),
            (
                "notes",
                json!([
                    "Words are counted within one column or fragment. A window stops at a gap, a lacuna marker or a verse-end sign (׃)."
                ]),
            ),
        ]);
        if ks.is_empty() {
            s.insert("suggestions".into(), json!(self.suggest(word, unit, 12)));
        }
        Ok(Answer { table, summary: s })
    }

    // ---- keyness ------------------------------------------------------------

    /// Words that are used much more (or less) in some scrolls than in others.
    ///
    /// `target` and `reference` are scroll filters like "1QS, 4Q26*". An empty reference means
    /// every other scroll (in the texts, language and reconstruction settings of `scope`; the
    /// scope's own scrolls setting is not used here). A scroll named in both counts as target.
    ///
    /// * `ll`: log-likelihood (G2) of the difference; higher means more certain it is not chance
    /// * `log_ratio`: log2 of (rate in target / rate in reference): +1 means twice as frequent in
    ///   the target, -1 half as frequent (a word missing from one side is treated as 0.5)
    ///
    /// `direction`: "over" (more in the target), "under" (less) or "both". `min_count` applies to
    /// the two sides together.
    #[allow(clippy::too_many_arguments)]
    pub fn keyness(
        &self,
        target: &str,
        reference: &str,
        scope: &Scope,
        unit: Unit,
        min_count: u64,
        sort: &str,
        direction: &str,
        top: usize,
        exclude: &str,
    ) -> StatsResult<Answer> {
        check("sort", sort, &KEYNESS_SORTS)?;
        check("direction", direction, &DIRECTIONS)?;
        let ids: Vec<String> = self.scrolls.iter().map(|s| s.id.clone()).collect();
        if target.trim().is_empty() {
            return fail("Name at least one scroll for the target, for example 1QS.");
        }
        let target_ids = select_scrolls(target, &ids).unwrap_or_default();
        if target_ids.is_empty() {
            return fail(format!("No scroll matches '{}'.", target.trim()));
        }
        let reference_ids = if reference.trim().is_empty() { None } else { select_scrolls(reference, &ids) };
        if let Some(list) = &reference_ids {
            if list.is_empty() {
                return fail(format!("No scroll matches '{}'.", reference.trim()));
            }
        }

        let everywhere = Scope { scrolls: String::new(), ..scope.clone() };
        let elig = self.eligible(&everywhere, unit);
        let mut is_target = vec![false; ids.len()];
        for &i in &target_ids {
            is_target[i] = true;
        }
        let mut is_reference = vec![reference_ids.is_none(); ids.len()];
        if let Some(list) = &reference_ids {
            for &i in list {
                is_reference[i] = true;
            }
        }

        let nk = self.key_count(unit);
        let (mut ta, mut tb) = (vec![0u32; nk], vec![0u32; nk]);
        let (mut pairs_a, mut pairs_b): (Vec<(u32, u32)>, Vec<(u32, u32)>) = (Vec::new(), Vec::new());
        let (mut na, mut nb) = (0usize, 0usize);
        let (mut scrolls_a, mut scrolls_b) = (HashSet::new(), HashSet::new());
        for e in &elig {
            if is_target[e.scroll as usize] {
                na += 1;
                ta[e.k as usize] += 1;
                pairs_a.push((e.k, e.scroll));
                scrolls_a.insert(e.scroll);
            } else if is_reference[e.scroll as usize] {
                nb += 1;
                tb[e.k as usize] += 1;
                pairs_b.push((e.k, e.scroll));
                scrolls_b.insert(e.scroll);
            }
        }
        if na == 0 {
            return fail("The target scrolls have no words under these settings.");
        }
        if nb == 0 {
            return fail("The reference scrolls have no words under these settings.");
        }

        // the number of different scrolls each word occurs in, on each side
        let distinct = |mut pairs: Vec<(u32, u32)>| -> Vec<u32> {
            pairs.sort_unstable();
            pairs.dedup();
            let mut counts = vec![0u32; nk];
            for (k, _) in pairs {
                counts[k as usize] += 1;
            }
            counts
        };
        let (sa_k, sb_k) = (distinct(pairs_a), distinct(pairs_b));

        struct Row {
            k: u32,
            target_pm: f64,
            reference_pm: f64,
            log_ratio: f64,
            ll: f64,
        }
        let labels = self.labels(unit);
        let (naf, nbf) = (na as f64, nb as f64);
        let skip = exclusion(exclude);
        let mut hidden = 0;
        let mut rows: Vec<Row> = Vec::new();
        for k in 0..nk {
            let (a, b) = (ta[k], tb[k]);
            if a == 0 && b == 0 {
                continue;
            }
            if ((a + b) as u64) < min_count {
                continue;
            }
            let (fa, fb) = (a as f64, b as f64);
            match direction {
                "over" if !(fa * nbf > fb * naf) => continue,
                "under" if !(fa * nbf < fb * naf) => continue,
                _ => {}
            }
            if skip.contains(&labels[k].item_norm) {
                hidden += 1;
                continue;
            }
            let floor_a = if fa > 0.0 { fa } else { 0.5 };
            let floor_b = if fb > 0.0 { fb } else { 0.5 };
            rows.push(Row {
                k: k as u32,
                target_pm: div_const(fa, naf) * 1e6,
                reference_pm: div_const(fb, nbf) * 1e6,
                log_ratio: log2(div_const(floor_a, naf) / div_const(floor_b, nbf)),
                ll: g2(fa, naf, fa + fb, naf + nbf),
            });
        }
        let key = |r: &Row| -> f64 {
            if sort == "ll" {
                r.ll
            } else if direction == "over" {
                r.log_ratio
            } else if direction == "under" {
                -r.log_ratio
            } else {
                r.log_ratio.abs()
            }
        };
        rows.sort_by(|a, b| {
            cmp_f64(key(b), key(a))
                .then_with(|| cmp_f64(b.ll, a.ll))
                .then_with(|| labels[a.k as usize].item.cmp(&labels[b.k as usize].item))
                .then(a.k.cmp(&b.k))
        });
        let matching = rows.len();
        rows.truncate(limit(top));

        let with_lang = unit == Unit::Lexeme;
        let mut columns = vec!["rank", "item"];
        if with_lang {
            columns.push("lang");
        }
        columns.extend(["target", "reference", "target_pm", "reference_pm", "log_ratio", "ll", "target_scrolls", "reference_scrolls"]);
        let mut table = Table::new(&columns);
        for (r, row) in rows.iter().enumerate() {
            let k = row.k as usize;
            let mut cells: Vec<Value> = vec![(r + 1).into(), labels[k].item.as_str().into()];
            if with_lang {
                cells.push(lang_value(&labels[k]));
            }
            cells.extend([
                ta[k].into(),
                tb[k].into(),
                row.target_pm.into(),
                row.reference_pm.into(),
                row.log_ratio.into(),
                row.ll.into(),
                sa_k[k].into(),
                sb_k[k].into(),
            ]);
            table.rows.push(cells);
        }

        let (sa, sb) = (scrolls_a.len(), scrolls_b.len());
        let plural = |n: usize| if n == 1 { "scroll" } else { "scrolls" };
        let text = format!(
            "Target: {} words in {} {}. Reference: {} words in {} {}.",
            fmt_int(na),
            fmt_int(sa),
            plural(sa),
            fmt_int(nb),
            fmt_int(sb),
            plural(sb)
        );
        let shown = table.len();
        Ok(Answer {
            summary: summary(vec![
                ("text", text.into()),
                ("target_words", na.into()),
                ("reference_words", nb.into()),
                ("target_scrolls", sa.into()),
                ("reference_scrolls", sb.into()),
                ("matching", matching.into()),
                ("shown", shown.into()),
                ("hidden", hidden.into()),
                ("direction", direction.into()),
            ]),
            table,
        })
    }
}
