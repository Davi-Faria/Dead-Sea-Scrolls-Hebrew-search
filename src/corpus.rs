//! The corpus: `data/dss.json` loaded into flat tables that the analyses run on.
//!
//! How words are counted (the choices that shape every number):
//!
//! * A "word" is a token with at least one Hebrew letter. Gap markers (…), the verse-end sign
//!   (׃) and lacuna markers (# [ ]) are not words. A word's *form* is its consonants only.
//! * Unit "lexeme" counts a word under the dataset's lexeme (words without one are left out);
//!   unit "form" counts the word as written, with final letters made ordinary (ן = נ), as the
//!   Search tab does.
//! * Collocation windows count words within one column or fragment. They continue across line
//!   breaks but stop at gaps, lacuna markers and the verse-end sign, because words on either
//!   side of a gap are not really next to each other. Every token gets a *segment* number and
//!   a place in it (`seg`, `idx`) for this purpose.

use crate::hebrew::{is_letter, normalize_finals};
use crate::table::RowWriter;
use serde::Deserialize;
use serde_json::Value as Json;
use std::collections::HashMap;
use std::io::{self, Write};

pub const BIT_REC_PART: u8 = 1; // some letters reconstructed by a modern editor
pub const BIT_REC_FULL: u8 = 2; // all letters reconstructed
pub const BIT_UNC: u8 = 4; // some letters uncertain
pub const BIT_COR: u8 = 8; // corrected (by an ancient or modern editor)
pub const BIT_REM: u8 = 16; // removed (by an ancient or modern editor)
pub const BIT_REC_ANY: u8 = BIT_REC_PART | BIT_REC_FULL;
pub const B32: &str = "0123456789abcdefghijklmnopqrstuv";

pub const NO_FORM: u32 = u32::MAX;
pub const NO_SEG: u32 = u32::MAX;

/// What a "word" is counted as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// every form of a word together, by the dataset's lexeme
    Lexeme,
    /// the word as written, with final letters made ordinary
    Form,
}

impl Unit {
    pub fn as_str(self) -> &'static str {
        match self {
            Unit::Lexeme => "lexeme",
            Unit::Form => "form",
        }
    }
}

/// One token of the text, in reading order. Its position in `Corpus::tokens` is its word id.
#[derive(Clone, Debug)]
pub struct Token {
    pub line: u32,
    pub pos: u16,
    pub scroll: u32,
    /// lexeme id, or -1 for a word without one
    pub lemma: i32,
    /// id of the word's spelling (final letters made ordinary), or NO_FORM when the token is not a word
    pub form: u32,
    /// segment of really adjacent words, and the word's place in it (NO_SEG for non-words)
    pub seg: u32,
    pub idx: u32,
    pub bits: u8,
    pub biblical: bool,
}

impl Token {
    #[inline]
    pub fn is_word(&self) -> bool {
        self.form != NO_FORM
    }
    #[inline]
    pub fn rec_part(&self) -> bool {
        self.bits & BIT_REC_PART != 0
    }
    #[inline]
    pub fn rec_full(&self) -> bool {
        self.bits & BIT_REC_FULL != 0
    }
}

#[derive(Clone, Debug)]
pub struct Line {
    pub scroll: u32,
    pub frag: String,
    pub lnum: String,
    pub biblical: bool,
    pub bible_ref: Option<String>,
    pub words: Vec<String>,
    /// word id of the line's first token
    pub first_token: u32,
}

#[derive(Clone, Debug)]
pub struct Scroll {
    pub id: String,
    pub name: String,
    pub cave: String,
    /// number of words (tokens with Hebrew letters), whatever the filters
    pub words: u32,
}

#[derive(Clone, Debug)]
pub struct Lemma {
    pub text: String,
    pub lang: String,
}

/// What to show for a countable word (and what to match a typed word against).
#[derive(Clone, Debug)]
pub struct Label {
    pub item: String,
    pub item_norm: String,
    /// language of a lexeme ("h", "a", "g"); None for spellings
    pub lang: Option<String>,
    /// occurrences in the whole corpus
    pub total: u32,
}

pub struct Corpus {
    pub meta: Json,
    pub scrolls: Vec<Scroll>,
    pub lemmas: Vec<Lemma>,
    pub lines: Vec<Line>,
    pub tokens: Vec<Token>,
    /// the spellings: final letters made ordinary, indexed by `Token::form`
    pub form_keys: Vec<String>,
    lex_labels: Vec<Label>,
    form_labels: Vec<Label>,
}

#[derive(Deserialize)]
struct RawLine(u32, String, String, Vec<String>, Vec<i32>, String, i64, String);

#[derive(Deserialize)]
struct RawData {
    #[serde(default)]
    meta: Json,
    scrolls: Vec<String>,
    lemmas: Vec<String>,
    #[serde(rename = "lemmaLang")]
    lemma_lang: Vec<String>,
    lines: Vec<RawLine>,
}

fn b32_value(c: char) -> u8 {
    B32.find(c).map(|i| i as u8).unwrap_or(0)
}

// --------------------------------------------------------------------------
// Scroll names: groups, order, filters
// --------------------------------------------------------------------------

/// '4Q266' -> '4Q'; 'CD' -> 'CD'; 'PAM43660' -> 'PAM' (the group a scroll belongs to).
pub fn cave_of(scroll_id: &str) -> String {
    let b = scroll_id.as_bytes();
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < b.len() && (b[digits] == b'Q' || b[digits] == b'q') {
        return format!("{}Q", &scroll_id[..digits]);
    }
    let letters = b.iter().take_while(|c| c.is_ascii_alphabetic()).count();
    if letters > 0 {
        scroll_id[..letters].to_string()
    } else {
        scroll_id.to_string()
    }
}

pub type SortKey = (u32, Vec<(u8, u64, String)>);

/// Order scroll ids by cave number, then naturally (4Q55 before 4Q56 before 4Q100).
pub fn scroll_sort_key(scroll_id: &str) -> SortKey {
    let b = scroll_id.as_bytes();
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    let cave = if digits > 0 && digits < b.len() && (b[digits] == b'Q' || b[digits] == b'q') {
        scroll_id[..digits].parse::<u32>().unwrap_or(u32::MAX)
    } else {
        999
    };
    let mut parts = Vec::new();
    let mut rest = scroll_id;
    while let Some(first) = rest.chars().next() {
        let numeric = first.is_ascii_digit();
        let end = rest.find(|c: char| c.is_ascii_digit() != numeric).unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        if numeric {
            parts.push((0u8, run.parse::<u64>().unwrap_or(u64::MAX), String::new()));
        } else {
            parts.push((1u8, 0u64, run.to_string()));
        }
        rest = tail;
    }
    (cave, parts)
}

/// Which scrolls does a filter like "1QS, CD, 4Q4*" select? (As in the Search tab.)
///
/// A name that matches a scroll exactly selects only that scroll (1QS does not include 1QSa);
/// a name ending in * selects every scroll that starts with it, and so does a name that
/// matches no scroll exactly. Matching ignores case. Returns None for an empty filter (all
/// scrolls), otherwise the sorted scroll numbers (possibly none).
pub fn select_scrolls(spec: &str, scroll_ids: &[String]) -> Option<Vec<usize>> {
    let tokens: Vec<&str> = spec.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return None;
    }
    let lower: Vec<String> = scroll_ids.iter().map(|s| s.to_lowercase()).collect();
    let mut chosen = std::collections::BTreeSet::new();
    for token in tokens {
        let mut t = token.to_lowercase();
        let prefix = t.ends_with('*');
        if prefix {
            t.pop();
        }
        let exact = !prefix && lower.iter().any(|n| *n == t);
        for (i, name) in lower.iter().enumerate() {
            let hit = if exact { *name == t } else { name.starts_with(&t) };
            if hit {
                chosen.insert(i);
            }
        }
    }
    Some(chosen.into_iter().collect())
}

// --------------------------------------------------------------------------
// Loading
// --------------------------------------------------------------------------

impl Corpus {
    /// Build the corpus from the bytes of `dss.json` and, if there is one, `scroll-names.json`.
    pub fn from_json(dss: &[u8], names: Option<&[u8]>) -> Result<Corpus, String> {
        let raw: RawData = serde_json::from_slice(dss).map_err(|e| format!("data/dss.json is not valid: {e}"))?;
        let names: HashMap<String, String> = match names {
            Some(bytes) => serde_json::from_slice(bytes).map_err(|e| format!("data/scroll-names.json is not valid: {e}"))?,
            None => HashMap::new(),
        };
        if raw.lemmas.len() != raw.lemma_lang.len() {
            return Err("data/dss.json: lemmas and lemmaLang differ in length".to_string());
        }

        let lemmas: Vec<Lemma> =
            raw.lemmas.iter().zip(&raw.lemma_lang).map(|(text, lang)| Lemma { text: text.clone(), lang: lang.clone() }).collect();

        let n_tokens: usize = raw.lines.iter().map(|l| l.3.len()).sum();
        let mut tokens: Vec<Token> = Vec::with_capacity(n_tokens);
        let mut lines: Vec<Line> = Vec::with_capacity(raw.lines.len());
        let mut form_ids: HashMap<String, u32> = HashMap::new();
        let mut form_keys: Vec<String> = Vec::new();
        let mut spellings: Vec<(u32, String)> = Vec::new();

        // Segments: runs of words that really are next to each other. A new one starts at a new
        // column or fragment, after anything that is not a word (a gap, a lacuna marker, ׃), and
        // where a word itself carries a marker before or after its letters.
        let mut seg_counter: u32 = 0;
        let mut idx: u32 = 0;
        // previous token: (scroll, fragment, is a word, its letters are followed by a marker)
        let mut prev: Option<(u32, &str, bool, bool)> = None;

        for (li, row) in raw.lines.iter().enumerate() {
            let RawLine(scroll, frag, lnum, words, lemma_ids, status, biblical, bible_ref) = row;
            if *scroll as usize >= raw.scrolls.len() {
                return Err(format!("data/dss.json: line {li} names scroll {scroll}, which does not exist"));
            }
            let status: Vec<u8> = status.chars().map(b32_value).chain(std::iter::repeat(0)).take(words.len()).collect();
            let first_token = tokens.len() as u32;
            for (pos, text) in words.iter().enumerate() {
                let letters: String = text.chars().filter(|&c| is_letter(c)).collect();
                let is_word = !letters.is_empty();
                let brk_before = is_word && !text.chars().next().is_some_and(is_letter);
                let brk_after = is_word && !text.chars().next_back().is_some_and(is_letter);
                let mut token = Token {
                    line: li as u32,
                    pos: pos as u16,
                    scroll: *scroll,
                    lemma: lemma_ids.get(pos).copied().unwrap_or(-1),
                    form: NO_FORM,
                    seg: NO_SEG,
                    idx: NO_SEG,
                    bits: status[pos],
                    biblical: *biblical != 0,
                };
                if is_word {
                    let key = normalize_finals(&letters);
                    let id = *form_ids.entry(key.clone()).or_insert_with(|| {
                        form_keys.push(key);
                        (form_keys.len() - 1) as u32
                    });
                    token.form = id;
                    spellings.push((id, letters));
                    let cut = brk_before
                        || match prev {
                            None => true,
                            Some((ps, pf, pword, pbrk)) => ps != *scroll || pf != frag.as_str() || !pword || pbrk,
                        };
                    if cut {
                        seg_counter += 1;
                        idx = 0;
                    } else {
                        idx += 1;
                    }
                    token.seg = seg_counter;
                    token.idx = idx;
                }
                prev = Some((*scroll, frag.as_str(), is_word, brk_after));
                tokens.push(token);
            }
            lines.push(Line {
                scroll: *scroll,
                frag: frag.clone(),
                lnum: lnum.clone(),
                biblical: *biblical != 0,
                bible_ref: if bible_ref.is_empty() { None } else { Some(bible_ref.clone()) },
                words: words.clone(),
                first_token,
            });
        }

        // scrolls
        let mut sizes = vec![0u32; raw.scrolls.len()];
        for t in &tokens {
            if t.is_word() {
                sizes[t.scroll as usize] += 1;
            }
        }
        let scrolls: Vec<Scroll> = raw
            .scrolls
            .iter()
            .enumerate()
            .map(|(i, id)| Scroll { id: id.clone(), name: names.get(id).cloned().unwrap_or_default(), cave: cave_of(id), words: sizes[i] })
            .collect();

        // labels: what to show for each countable word
        let mut lex_total = vec![0u32; lemmas.len()];
        let mut form_total = vec![0u32; form_keys.len()];
        for t in &tokens {
            if t.is_word() {
                form_total[t.form as usize] += 1;
                if t.lemma >= 0 && (t.lemma as usize) < lemmas.len() {
                    lex_total[t.lemma as usize] += 1;
                }
            }
        }
        let lex_labels: Vec<Label> = lemmas
            .iter()
            .zip(&lex_total)
            .map(|(l, &total)| Label { item: l.text.clone(), item_norm: normalize_finals(&l.text), lang: Some(l.lang.clone()), total })
            .collect();

        // a spelling is shown as its most common written form (the first in letter order on a tie)
        spellings.sort_unstable();
        let mut shown: Vec<Option<String>> = vec![None; form_keys.len()];
        let mut i = 0;
        while i < spellings.len() {
            let id = spellings[i].0;
            let mut best: Option<(usize, &str)> = None;
            while i < spellings.len() && spellings[i].0 == id {
                let start = i;
                while i < spellings.len() && spellings[i] == spellings[start] {
                    i += 1;
                }
                let count = i - start;
                if best.is_none_or(|(c, _)| count > c) {
                    best = Some((count, spellings[start].1.as_str()));
                }
            }
            shown[id as usize] = best.map(|(_, s)| s.to_string());
        }
        let form_labels: Vec<Label> = form_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let item = shown[i].clone().unwrap_or_else(|| key.clone());
                Label { item_norm: normalize_finals(&item), item, lang: None, total: form_total[i] }
            })
            .collect();

        Ok(Corpus { meta: raw.meta, scrolls, lemmas, lines, tokens, form_keys, lex_labels, form_labels })
    }

    pub fn labels(&self, unit: Unit) -> &[Label] {
        match unit {
            Unit::Lexeme => &self.lex_labels,
            Unit::Form => &self.form_labels,
        }
    }

    /// How many different words there are to count under a unit (the size of the label table).
    pub fn key_count(&self, unit: Unit) -> usize {
        self.labels(unit).len()
    }

    /// The key that identifies a token's word under a unit: its lexeme id, or its spelling id.
    /// None for tokens that are not words, and (for lexemes) for words without a lexeme.
    #[inline]
    pub fn key(&self, unit: Unit, t: &Token) -> Option<u32> {
        if !t.is_word() {
            return None;
        }
        match unit {
            Unit::Form => Some(t.form),
            Unit::Lexeme => (t.lemma >= 0 && (t.lemma as usize) < self.lemmas.len()).then_some(t.lemma as u32),
        }
    }

    /// 'a' for Aramaic words; a word without a lexeme counts as Hebrew, as in the Search tab.
    #[inline]
    pub fn is_aramaic(&self, t: &Token) -> bool {
        t.lemma >= 0 && self.lemmas.get(t.lemma as usize).is_some_and(|l| l.lang == "a")
    }

    pub fn word_lang(&self, t: &Token) -> &str {
        if t.lemma >= 0 {
            self.lemmas.get(t.lemma as usize).map(|l| l.lang.as_str()).unwrap_or("h")
        } else {
            "h"
        }
    }

    /// The token as exported (prefix and stem joined; may carry a gap marker).
    pub fn token_text(&self, t: &Token) -> &str {
        &self.lines[t.line as usize].words[t.pos as usize]
    }

    /// A short description of the corpus, for the command line.
    pub fn info(&self) -> Vec<(&'static str, String)> {
        let words = self.tokens.iter().filter(|t| t.is_word());
        let mut with_lexeme = 0;
        let mut aramaic = 0;
        let mut biblical = 0;
        let mut reconstructed = 0;
        let mut total = 0;
        for t in words {
            total += 1;
            if t.lemma >= 0 {
                with_lexeme += 1;
            }
            if self.is_aramaic(t) {
                aramaic += 1;
            }
            if t.biblical {
                biblical += 1;
            }
            if t.bits & BIT_REC_ANY != 0 {
                reconstructed += 1;
            }
        }
        let text = |key: &str| self.meta.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string();
        vec![
            ("tokens", self.tokens.len().to_string()),
            ("words", total.to_string()),
            ("words_with_lexeme", with_lexeme.to_string()),
            ("aramaic_words", aramaic.to_string()),
            ("lines", self.lines.len().to_string()),
            ("scrolls", self.scrolls.iter().filter(|s| s.words > 0).count().to_string()),
            ("lexemes", self.lemmas.len().to_string()),
            ("biblical_words", biblical.to_string()),
            ("reconstructed_words", reconstructed.to_string()),
            ("source", text("source")),
            ("license", text("license")),
        ]
    }

    // ---- tables for export -------------------------------------------------

    pub fn frame_columns(name: &str) -> Result<Vec<String>, String> {
        let cols: &[&str] = match name {
            "words" => &[
                "word_id",
                "line",
                "pos",
                "scroll",
                "scroll_id",
                "frag",
                "lnum",
                "text",
                "lemma_id",
                "lemma",
                "lang",
                "bits",
                "biblical",
                "form",
                "is_word",
                "form_key",
                "rec_part",
                "rec_full",
                "unc",
                "cor",
                "rem",
                "seg",
                "idx",
            ],
            "lines" => &["line", "scroll", "scroll_id", "frag", "lnum", "biblical", "bible_ref", "n_words", "text"],
            "lemmas" => &["lemma_id", "lemma", "lang"],
            "scrolls" => &["scroll", "scroll_id", "name", "cave", "words"],
            other => return Err(format!("Unknown table '{other}'; choose one of words, lines, lemmas, scrolls.")),
        };
        Ok(cols.iter().map(|c| c.to_string()).collect())
    }

    /// Number of rows of an export table.
    pub fn frame_len(&self, name: &str) -> usize {
        match name {
            "words" => self.tokens.len(),
            "lines" => self.lines.len(),
            "lemmas" => self.lemmas.len(),
            _ => self.scrolls.len(),
        }
    }

    /// Write one of the corpus tables (words, lines, lemmas, scrolls), row by row.
    pub fn write_frame<W: Write>(&self, name: &str, out: &mut RowWriter<W>) -> io::Result<()> {
        match name {
            "words" => {
                for (id, t) in self.tokens.iter().enumerate() {
                    let text = self.token_text(t);
                    let form: String = text.chars().filter(|&c| is_letter(c)).collect();
                    let lemma = (t.lemma >= 0).then(|| self.lemmas.get(t.lemma as usize)).flatten();
                    let is_word = t.is_word();
                    let line = &self.lines[t.line as usize];
                    out.write_row(&[
                        id.into(),
                        t.line.into(),
                        t.pos.into(),
                        t.scroll.into(),
                        self.scrolls[t.scroll as usize].id.as_str().into(),
                        line.frag.as_str().into(),
                        line.lnum.as_str().into(),
                        text.into(),
                        lemma.map(|_| t.lemma).into(),
                        lemma.map(|l| l.text.clone()).into(),
                        self.word_lang(t).into(),
                        t.bits.into(),
                        t.biblical.into(),
                        form.as_str().into(),
                        is_word.into(),
                        normalize_finals(&form).into(),
                        t.rec_part().into(),
                        t.rec_full().into(),
                        (t.bits & BIT_UNC != 0).into(),
                        (t.bits & BIT_COR != 0).into(),
                        (t.bits & BIT_REM != 0).into(),
                        is_word.then_some(t.seg).into(),
                        is_word.then_some(t.idx).into(),
                    ])?;
                }
            }
            "lines" => {
                for (i, l) in self.lines.iter().enumerate() {
                    out.write_row(&[
                        i.into(),
                        l.scroll.into(),
                        self.scrolls[l.scroll as usize].id.as_str().into(),
                        l.frag.as_str().into(),
                        l.lnum.as_str().into(),
                        l.biblical.into(),
                        l.bible_ref.clone().into(),
                        l.words.len().into(),
                        l.words.join(" ").into(),
                    ])?;
                }
            }
            "lemmas" => {
                for (i, l) in self.lemmas.iter().enumerate() {
                    out.write_row(&[i.into(), l.text.as_str().into(), l.lang.as_str().into()])?;
                }
            }
            _ => {
                for (i, s) in self.scrolls.iter().enumerate() {
                    out.write_row(&[i.into(), s.id.as_str().into(), s.name.as_str().into(), s.cave.as_str().into(), s.words.into()])?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn caves() {
        assert_eq!(cave_of("4Q266"), "4Q");
        assert_eq!(cave_of("1QHa"), "1Q");
        assert_eq!(cave_of("CD"), "CD");
        assert_eq!(cave_of("PAM43660"), "PAM");
        assert_eq!(cave_of("11q5"), "11Q");
    }

    #[test]
    fn scrolls_sort_by_cave_then_naturally() {
        let mut v = ids(&["4Q100", "CD", "4Q55", "1QS", "11Q5", "4Q56", "PAM43660"]);
        v.sort_by_key(|s| scroll_sort_key(s));
        assert_eq!(v, ids(&["1QS", "4Q55", "4Q56", "4Q100", "11Q5", "CD", "PAM43660"]));
    }

    #[test]
    fn scroll_filters() {
        let all = ids(&["1QS", "1QSa", "1QSb", "CD", "4Q266", "4Q267", "4Q41"]);
        assert_eq!(select_scrolls("", &all), None);
        assert_eq!(select_scrolls("1QS", &all), Some(vec![0]));
        assert_eq!(select_scrolls("1qs*", &all), Some(vec![0, 1, 2]));
        assert_eq!(select_scrolls("4Q26", &all), Some(vec![4, 5]));
        assert_eq!(select_scrolls("1QS, CD;4Q41", &all), Some(vec![0, 3, 6]));
        assert_eq!(select_scrolls("nope", &all), Some(vec![]));
    }

    fn tiny() -> Corpus {
        // one scroll, one fragment, two lines; the second line starts after a gap
        let json = r#"{
          "meta": {"source": "test", "license": "none"},
          "scrolls": ["1QS"],
          "lemmas": ["אב", "בן"],
          "lemmaLang": ["h", "h"],
          "lines": [
            [0, "1", "1", ["אב", "בן", "…", "אב"], [0, 1, -1, 0], "0200", 0, ""],
            [0, "1", "2", ["בן", "אב"], [1, 0], "00", 0, ""]
          ]
        }"#;
        Corpus::from_json(json.as_bytes(), None).unwrap()
    }

    #[test]
    fn segments_stop_at_gaps_but_cross_line_breaks() {
        let c = tiny();
        let seg: Vec<(u32, u32)> = c.tokens.iter().map(|t| (t.seg, t.idx)).collect();
        assert_eq!(seg[0], (1, 0));
        assert_eq!(seg[1], (1, 1));
        assert_eq!(seg[2], (NO_SEG, NO_SEG)); // the gap is not a word
        assert_eq!(seg[3], (2, 0)); // a new segment after the gap
        assert_eq!(seg[4], (2, 1)); // the next line continues it
        assert_eq!(seg[5], (2, 2));
        assert_eq!(c.tokens[1].bits, 2);
    }

    #[test]
    fn labels_count_the_whole_corpus() {
        let c = tiny();
        assert_eq!(c.labels(Unit::Lexeme)[0].total, 3);
        assert_eq!(c.labels(Unit::Lexeme)[1].total, 2);
        assert_eq!(c.labels(Unit::Form).len(), 2);
        assert_eq!(c.scrolls[0].words, 5);
    }
}
