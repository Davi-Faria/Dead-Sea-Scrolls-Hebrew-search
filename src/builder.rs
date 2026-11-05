//! Build `data/dss.json` and `data/bible.json` from the ETCBC/CACCHT Dead Sea Scrolls dataset.
//!
//! You only need this if you want to rebuild the data (for a newer version of the dataset, or
//! after changing what is exported). The program ships with the data already built.
//!
//! Output format: see "Data format" in README.md.

use crate::bible::{self, Segment, BOOKS};
use crate::corpus::{Line, B32, BIT_COR, BIT_REC_FULL, BIT_REC_PART, BIT_REM, BIT_UNC};
use crate::hebrew::hebrew_letters;
use crate::tf::{self, Feature, Slots};
use serde_json::{json, Map, Value as Json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The dataset commit that the shipped data was built from. Change this (or pass another
/// `--ref`) to build from a newer version.
pub const DEFAULT_REF: &str = "ac91a978d630f426e327f88eb0eeb85d6e67210a";
pub const DEFAULT_VERSION: &str = "2.0.1";
pub const RAW_URL: &str = "https://raw.githubusercontent.com/ETCBC/dss/{ref}/tf/{version}/{name}.tf";

/// Text-Fabric needs these two to understand the corpus structure.
pub const CORE_FILES: [&str; 2] = ["otype", "oslots"];
/// The features this reads (see docs/feature_documentation.md in ETCBC/dss).
pub const FEATURES: [&str; 18] = [
    "scroll", "fragment", "line", // location: scroll, fragment/column, line
    "biblical", "book", "chapter", "verse", // biblical material and its reference
    "glyph", "full", // the text of a word
    "glex", "sp", // lexeme (pointed Hebrew) and part of speech
    "lang", "after", "type", // language, space-after, kind of sign
    "rec", "unc", "cor", "rem", // editorial marks on signs
];

// --------------------------------------------------------------------------
// Pure helpers (covered by the tests below)
// --------------------------------------------------------------------------

/// (book, chapter, verse) as numbers for a placed biblical passage, else None. Other values of
/// the dataset's `book` feature (for example "1Q4") are manuscripts it could not place.
pub fn bible_ref(book: Option<&str>, chapter: Option<&str>, verse: Option<&str>) -> Option<(String, u32, u32)> {
    let book = book?;
    let digits = |s: Option<&str>| s.filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())).and_then(|s| s.parse::<u32>().ok());
    if BOOKS.iter().any(|(code, _)| *code == book) {
        Some((book.to_string(), digits(chapter)?, digits(verse)?))
    } else {
        None
    }
}

/// The text of one token as shown on the page.
pub fn display_text(glyph: Option<&str>, full: Option<&str>, lang: Option<&str>) -> String {
    let raw = glyph.filter(|s| !s.is_empty()).or(full.filter(|s| !s.is_empty())).unwrap_or("");
    if lang == Some("g") {
        // a handful of Greek tokens
        return raw.trim().to_string();
    }
    let letters = hebrew_letters(raw);
    if !letters.is_empty() {
        return letters;
    }
    if raw.contains('\u{03b5}') || raw.trim().is_empty() {
        // ε marks a missing sign
        return "\u{2026}".to_string();
    }
    raw.trim().to_string() // punctuation such as the sof pasuq
}

#[derive(Clone, Debug, Default)]
pub struct Token {
    pub text: String,
    pub lemma: Option<String>,
    pub sp: Option<String>,
    pub lang: Option<String>,
    pub bits: u8,
    /// None = no space follows
    pub after: Option<String>,
    pub reference: Option<(String, u32, u32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub lemma: Option<String>,
    /// 'h', 'a' or 'g'
    pub lang: char,
    pub bits: u8,
    pub reference: Option<(String, u32, u32)>,
}

/// Combine tokens written together (e.g. prefix + stem) into one orthographic word.
pub fn merge_tokens(parts: &[Token]) -> Word {
    let text: String = parts.iter().map(|p| p.text.as_str()).collect();
    let stem = parts.iter().find(|p| p.sp.as_deref() != Some("ptcl") && p.lemma.as_ref().is_some_and(|l| !l.is_empty()));
    let lang_source = stem.unwrap_or(&parts[0]);
    let lang = match lang_source.lang.as_deref() {
        Some("a") => 'a',
        Some("g") => 'g',
        _ => 'h',
    };

    let mut bits = 0u8;
    for p in parts {
        bits |= p.bits & (BIT_UNC | BIT_COR | BIT_REM);
    }
    let rec_flags: Vec<u8> =
        parts.iter().filter(|p| !hebrew_letters(&p.text).is_empty()).map(|p| p.bits & (BIT_REC_PART | BIT_REC_FULL)).collect();
    if rec_flags.iter().any(|&f| f != 0) {
        let all_full = rec_flags.iter().all(|&f| f & BIT_REC_FULL != 0);
        bits |= if all_full { BIT_REC_FULL } else { BIT_REC_PART };
    }

    // Bible reference of the word, from the stem or the first part that has one.
    let reference = stem.and_then(|s| s.reference.clone()).or_else(|| parts.iter().find_map(|p| p.reference.clone()));
    Word { text, lemma: stem.and_then(|s| s.lemma.clone()), lang, bits, reference }
}

/// Runs of words that share a Bible reference.
pub fn verse_segments(line_index: u32, words: &[Word]) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut current: Option<&(String, u32, u32)> = None;
    let mut push = |current: &(String, u32, u32), start: usize, end: usize| {
        out.push(Segment {
            line: line_index,
            book: current.0.clone(),
            chapter: current.1,
            verse: current.2,
            start: start as u32,
            end: end as u32,
        });
    };
    for (i, word) in words.iter().enumerate() {
        let reference = word.reference.as_ref();
        if reference != current {
            if let Some(c) = current {
                push(c, start, i);
            }
            current = reference;
            start = i;
        }
    }
    if let Some(c) = current {
        push(c, start, words.len());
    }
    out
}

/// Group the tokens of one line into orthographic words. The dataset writes prefixes (ו ה ב כ ל
/// מ ש) as separate tokens with no space after them. A token with a space after it ends the word.
pub fn group_tokens(tokens: &[Token]) -> Vec<Word> {
    let mut words = Vec::new();
    let mut start = 0;
    for (i, token) in tokens.iter().enumerate() {
        if token.after.is_some() {
            words.push(merge_tokens(&tokens[start..=i]));
            start = i + 1;
        }
    }
    if start < tokens.len() {
        words.push(merge_tokens(&tokens[start..]));
    }
    words
}

// --------------------------------------------------------------------------
// Building
// --------------------------------------------------------------------------

/// What `build` makes.
pub struct Built {
    /// the contents of dss.json
    pub data: Json,
    /// the verse segments, for bible.json
    pub segments: Vec<Segment>,
    /// the lines, in the form the quotation search wants
    pub lines: Vec<Line>,
    pub words: usize,
}

/// A group of nodes of one type, in the order of their first slot.
struct Level {
    nodes: Vec<u32>,
    first: Vec<u32>,
}

impl Level {
    fn new(range: (u32, u32), slots: &Slots) -> Level {
        let mut nodes: Vec<u32> = (range.0..=range.1).filter(|&n| !slots.get(n).is_empty()).collect();
        // canonical order: by first slot, and a larger node before a smaller one that starts together
        nodes.sort_by_key(|&n| {
            let r = slots.get(n);
            (r[0].0, std::cmp::Reverse(r[r.len() - 1].1), n)
        });
        let first = nodes.iter().map(|&n| slots.get(n)[0].0).collect();
        Level { nodes, first }
    }
}

fn within(child: &[(u32, u32)], parent: &[(u32, u32)]) -> bool {
    child.iter().all(|&(a, b)| parent.iter().any(|&(c, d)| c <= a && b <= d))
}

/// For each node of `parents`, the nodes of `children` that lie inside it, in order.
fn nest(parents: &Level, children: &Level, slots: &Slots) -> HashMap<u32, Vec<u32>> {
    let mut out: HashMap<u32, Vec<u32>> = HashMap::new();
    for &child in &children.nodes {
        let cs = slots.get(child);
        // the last parent that starts at or before the child
        let at = parents.first.partition_point(|&f| f <= cs[0].0);
        if at == 0 {
            continue;
        }
        let parent = parents.nodes[at - 1];
        if within(cs, slots.get(parent)) {
            out.entry(parent).or_default().push(child);
        }
    }
    out
}

fn type_range(types: &[(u32, u32, String)], name: &str) -> Result<(u32, u32), String> {
    types.iter().find(|(_, _, t)| t == name).map(|&(a, b, _)| (a, b)).ok_or_else(|| format!("The dataset has no nodes of type '{name}'."))
}

/// Build the data from a folder with the `.tf` files. `progress` gets messages for the person running it.
pub fn build(tf_dir: &Path, reference: &str, progress: &dyn Fn(&str)) -> Result<Built, String> {
    progress("Loading Text-Fabric data ...");
    let types = tf::read_otype(&tf_dir.join("otype.tf"))?;
    let slots_range = type_range(&types, "sign")?;
    let (word_r, line_r, frag_r, scroll_r) =
        (type_range(&types, "word")?, type_range(&types, "line")?, type_range(&types, "fragment")?, type_range(&types, "scroll")?);
    let (low, high) = (frag_r.0.min(line_r.0).min(scroll_r.0).min(word_r.0), frag_r.1.max(line_r.1).max(scroll_r.1).max(word_r.1));
    let is_structure = |n: u32| {
        (word_r.0..=word_r.1).contains(&n)
            || (line_r.0..=line_r.1).contains(&n)
            || (frag_r.0..=frag_r.1).contains(&n)
            || (scroll_r.0..=scroll_r.1).contains(&n)
    };

    let node = |name: &str| tf::read_feature(&tf_dir.join(format!("{name}.tf")), low, high);
    let sign = |name: &str| tf::read_feature(&tf_dir.join(format!("{name}.tf")), slots_range.0, slots_range.1);
    let (f_scroll, f_fragment, f_line): (Feature, Feature, Feature) = (node("scroll")?, node("fragment")?, node("line")?);
    let (f_biblical, f_book, f_chapter, f_verse) = (node("biblical")?, node("book")?, node("chapter")?, node("verse")?);
    let (f_glyph, f_full, f_glex, f_sp, f_lang, f_after) =
        (node("glyph")?, node("full")?, node("glex")?, node("sp")?, node("lang")?, node("after")?);
    let (f_type, rec, unc, cor, rem) = (sign("type")?, sign("rec")?, sign("unc")?, sign("cor")?, sign("rem")?);
    let slots = tf::read_oslots(&tf_dir.join("oslots.tf"), low, high, is_structure)?;

    // scroll > fragment > line > word, by which slots they cover
    let scrolls_level = Level::new(scroll_r, &slots);
    let fragments_level = Level::new(frag_r, &slots);
    let lines_level = Level::new(line_r, &slots);
    let words_level = Level::new(word_r, &slots);
    let fragments_of = nest(&scrolls_level, &fragments_level, &slots);
    let lines_of = nest(&fragments_level, &lines_level, &slots);
    let words_of = nest(&lines_level, &words_level, &slots);

    let token = |w: u32| -> Token {
        let word_slots = slots.get(w);
        let all: Vec<u32> = word_slots.iter().flat_map(|&(a, b)| a..=b).collect();
        let letters: Vec<u32> = all.iter().copied().filter(|&s| f_type.get(s) == Some("cons")).collect();
        let mut bits = 0u8;
        let n_rec = letters.iter().filter(|&&s| rec.has(s)).count();
        if n_rec > 0 {
            bits |= if n_rec == letters.len() { BIT_REC_FULL } else { BIT_REC_PART };
        }
        if all.iter().any(|&s| unc.has(s)) {
            bits |= BIT_UNC;
        }
        if all.iter().any(|&s| cor.has(s)) {
            bits |= BIT_COR;
        }
        if all.iter().any(|&s| rem.has(s)) {
            bits |= BIT_REM;
        }
        Token {
            text: display_text(f_glyph.get(w), f_full.get(w), f_lang.get(w)),
            lemma: f_glex.get(w).map(hebrew_letters),
            sp: f_sp.get(w).map(str::to_string),
            lang: f_lang.get(w).map(str::to_string),
            bits,
            after: f_after.get(w).map(str::to_string),
            reference: bible_ref(f_book.get(w), f_chapter.get(w), f_verse.get(w)),
        }
    };

    let mut scrolls: Vec<String> = Vec::new();
    let mut lemma_ids: HashMap<(String, char), usize> = HashMap::new();
    let mut lemma_list: Vec<(String, char)> = Vec::new();
    let mut lines_json: Vec<Json> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    let mut segments: Vec<Segment> = Vec::new();
    let mut n_words = 0usize;
    let no_nodes: Vec<u32> = Vec::new();

    progress("Converting scrolls ...");
    // the scrolls in node order, as the dataset lists them
    for sc in scroll_r.0..=scroll_r.1 {
        let scroll_index = scrolls.len() as u32;
        scrolls.push(f_scroll.get(sc).unwrap_or("").to_string());
        for &frag in fragments_of.get(&sc).unwrap_or(&no_nodes) {
            let frag_label = f_fragment.get(frag).unwrap_or("").to_string();
            for &ln in lines_of.get(&frag).unwrap_or(&no_nodes) {
                let word_nodes = words_of.get(&ln).unwrap_or(&no_nodes);
                let tokens: Vec<Token> = word_nodes.iter().map(|&w| token(w)).collect();
                if tokens.is_empty() {
                    continue;
                }
                let words = group_tokens(&tokens);
                let mut lemma_column: Vec<i64> = Vec::with_capacity(words.len());
                for word in &words {
                    match word.lemma.as_ref().filter(|l| !l.is_empty()) {
                        Some(lemma) => {
                            let key = (lemma.clone(), word.lang);
                            let id = *lemma_ids.entry(key.clone()).or_insert_with(|| {
                                lemma_list.push(key);
                                lemma_list.len() - 1
                            });
                            lemma_column.push(id as i64);
                        }
                        None => lemma_column.push(-1),
                    }
                }
                let biblical = f_biblical.get(ln).is_some_and(|v| v.trim().parse::<i64>().map(|n| n != 0).unwrap_or(!v.is_empty()));
                let mut bible_reference = String::new();
                if biblical {
                    if let Some(&first) = word_nodes.iter().find(|&&w| f_book.get(w).is_some()) {
                        let show = |v: Option<&str>| v.unwrap_or("None").to_string();
                        bible_reference =
                            format!("{} {}:{}", show(f_book.get(first)), show(f_chapter.get(first)), show(f_verse.get(first)));
                    }
                }
                segments.extend(verse_segments(lines_json.len() as u32, &words));
                let status: String = words.iter().map(|w| B32.as_bytes()[w.bits as usize] as char).collect();
                let line_label = f_line.get(ln).unwrap_or("").to_string();
                lines_json.push(json!([
                    scroll_index,
                    frag_label,
                    line_label,
                    words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
                    lemma_column,
                    status,
                    biblical as u8,
                    bible_reference
                ]));
                lines.push(Line {
                    scroll: scroll_index,
                    frag: frag_label.clone(),
                    lnum: f_line.get(ln).unwrap_or("").to_string(),
                    biblical,
                    bible_ref: if bible_reference.is_empty() { None } else { Some(bible_reference.clone()) },
                    words: words.iter().map(|w| w.text.clone()).collect(),
                    first_token: n_words as u32,
                });
                n_words += words.len();
            }
        }
    }

    let mut meta = Map::new();
    meta.insert("source".into(), json!("CACCHT Dead Sea Scrolls dataset (ETCBC/dss), based on data files of Martin G. Abegg"));
    meta.insert("url".into(), json!("https://github.com/ETCBC/dss"));
    meta.insert("ref".into(), json!(reference));
    meta.insert("license".into(), json!("CC BY-NC 4.0"));
    meta.insert("words".into(), json!(n_words));
    meta.insert("lines".into(), json!(lines_json.len()));
    let mut data = Map::new();
    data.insert("meta".into(), Json::Object(meta));
    data.insert("scrolls".into(), json!(scrolls));
    data.insert("lemmas".into(), json!(lemma_list.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>()));
    data.insert("lemmaLang".into(), json!(lemma_list.iter().map(|(_, g)| g.to_string()).collect::<Vec<_>>()));
    data.insert("lines".into(), Json::Array(lines_json));
    Ok(Built { data: Json::Object(data), segments, lines, words: n_words })
}

/// Build everything and write `dss.json` and, next to it, `bible.json`.
pub fn build_and_write(tf_dir: &Path, reference: &str, out: &Path, progress: &dyn Fn(&str)) -> Result<(), String> {
    let built = build(tf_dir, reference, progress)?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let write = |path: &Path, value: &Json| -> Result<f64, String> {
        let text = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        std::fs::write(path, &text).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
        Ok(text.len() as f64 / 1e6)
    };
    let size = write(out, &built.data)?;
    let lines = built.data["lines"].as_array().map(|l| l.len()).unwrap_or(0);
    progress(&format!(
        "Wrote {} ({size:.1} MB): {} words in {} lines, {} scrolls, {} lexemes.",
        out.display(),
        crate::stats::fmt_int(built.words),
        crate::stats::fmt_int(lines),
        built.data["scrolls"].as_array().map(|l| l.len()).unwrap_or(0),
        crate::stats::fmt_int(built.data["lemmas"].as_array().map(|l| l.len()).unwrap_or(0)),
    ));

    // Data for the Parallels tab: verse segments and detected quotations.
    let bible = bible::build_bible(&built.lines, &built.segments);
    let bible_out: PathBuf = out.with_file_name("bible.json");
    let size = write(&bible_out, &bible)?;
    progress(&format!(
        "Wrote {} ({size:.1} MB): {} verse segments in {} books, {} quotations found in other texts.",
        bible_out.display(),
        crate::stats::fmt_int(bible["meta"]["segments"].as_u64().unwrap_or(0) as usize),
        bible["books"].as_array().map(|b| b.len()).unwrap_or(0),
        crate::stats::fmt_int(bible["meta"]["quotes"].as_u64().unwrap_or(0) as usize),
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str, lemma: Option<&str>, sp: &str, after: Option<&str>) -> Token {
        Token {
            text: text.into(),
            lemma: lemma.map(str::to_string),
            sp: Some(sp.into()),
            lang: Some("h".into()),
            after: after.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn placed_passages_only() {
        assert_eq!(bible_ref(Some("Is"), Some("61"), Some("1")), Some(("Is".to_string(), 61, 1)));
        assert_eq!(bible_ref(Some("1Q4"), None, None), None);
        assert_eq!(bible_ref(Some("Is"), Some("x"), Some("1")), None);
        assert_eq!(bible_ref(None, Some("1"), Some("1")), None);
    }

    #[test]
    fn display_text_covers_the_odd_tokens() {
        assert_eq!(display_text(Some("עֵין"), None, Some("h")), "עין");
        assert_eq!(display_text(None, Some("\u{03b5}"), Some("h")), "\u{2026}");
        assert_eq!(display_text(Some("  "), None, None), "\u{2026}");
        assert_eq!(display_text(Some("׃"), None, None), "׃");
        assert_eq!(display_text(Some(" abc "), None, Some("g")), "abc");
    }

    #[test]
    fn prefixes_are_joined_to_their_word() {
        let tokens = vec![
            token("ו", Some("ו"), "ptcl", None),
            token("עתה", Some("עתה"), "advb", Some(" ")),
            token("כל", Some("כל"), "subs", Some(" ")),
        ];
        let words = group_tokens(&tokens);
        assert_eq!(words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), vec!["ועתה", "כל"]);
        assert_eq!(words[0].lemma.as_deref(), Some("עתה")); // the stem, not the prefix
    }

    #[test]
    fn reconstruction_flags_are_combined() {
        let mut a = token("ו", Some("ו"), "ptcl", None);
        let mut b = token("עתה", Some("עתה"), "advb", Some(" "));
        a.bits = BIT_REC_FULL;
        b.bits = BIT_REC_FULL;
        assert_eq!(merge_tokens(&[a.clone(), b.clone()]).bits, BIT_REC_FULL);
        b.bits = 0;
        assert_eq!(merge_tokens(&[a, b]).bits, BIT_REC_PART);
    }

    #[test]
    fn verse_segments_follow_the_reference() {
        let w = |reference: Option<(&str, u32, u32)>| Word {
            text: "x".into(),
            lemma: None,
            lang: 'h',
            bits: 0,
            reference: reference.map(|(b, c, v)| (b.to_string(), c, v)),
        };
        let words = vec![w(None), w(Some(("Gen", 1, 1))), w(Some(("Gen", 1, 1))), w(Some(("Gen", 1, 2))), w(None)];
        let segments = verse_segments(7, &words);
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].verse, segments[0].start, segments[0].end), (1, 1, 3));
        assert_eq!((segments[1].verse, segments[1].start, segments[1].end), (2, 3, 4));
        assert_eq!(segments[0].line, 7);
    }
}
