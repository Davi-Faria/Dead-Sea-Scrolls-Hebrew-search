//! Data for the Parallels tab: which manuscripts contain each biblical chapter, and where
//! other texts (pesharim, the Melchizedek text, rewritten Torah and so on) quote it.
//!
//! Two kinds of data are produced:
//!
//! * "segments": every run of words in a biblical manuscript that belongs to one verse, as
//!   `[line, book, chapter, verse, first word, end word]`. The page uses them to line
//!   manuscripts up verse by verse.
//! * "quotes": stretches of non-biblical text that match a biblical verse for at least four
//!   words in a row. This is a heuristic (see `find_quotes`), not a scholarly identification.

use crate::corpus::Line;
use crate::hebrew::skeleton;
use serde_json::{json, Map, Value as Json};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Book codes (the dataset's) and their names, in canonical order.
pub const BOOKS: [(&str, &str); 39] = [
    ("Gen", "Genesis"),
    ("Ex", "Exodus"),
    ("Lev", "Leviticus"),
    ("Num", "Numbers"),
    ("Deut", "Deuteronomy"),
    ("Josh", "Joshua"),
    ("Judg", "Judges"),
    ("1Sam", "1 Samuel"),
    ("2Sam", "2 Samuel"),
    ("1Kgs", "1 Kings"),
    ("2Kgs", "2 Kings"),
    ("Is", "Isaiah"),
    ("Jer", "Jeremiah"),
    ("Ezek", "Ezekiel"),
    ("Hos", "Hosea"),
    ("Joel", "Joel"),
    ("Amos", "Amos"),
    ("Obad", "Obadiah"),
    ("Jonah", "Jonah"),
    ("Mic", "Micah"),
    ("Nah", "Nahum"),
    ("Hab", "Habakkuk"),
    ("Zeph", "Zephaniah"),
    ("Hag", "Haggai"),
    ("Zech", "Zechariah"),
    ("Mal", "Malachi"),
    ("Ps", "Psalms"),
    ("Job", "Job"),
    ("Prov", "Proverbs"),
    ("Ruth", "Ruth"),
    ("Song", "Song of Songs"),
    ("Eccl", "Ecclesiastes"),
    ("Lam", "Lamentations"),
    ("Esth", "Esther"),
    ("Dan", "Daniel"),
    ("Ezra", "Ezra"),
    ("Neh", "Nehemiah"),
    ("1Chr", "1 Chronicles"),
    ("2Chr", "2 Chronicles"),
];

/// A run of words of one line that belongs to one verse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub line: u32,
    pub book: String,
    pub chapter: u32,
    pub verse: u32,
    pub start: u32,
    pub end: u32,
}

/// A stretch of non-biblical text that matches biblical verses of one chapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quote {
    pub line: u32,
    pub start: u32,
    pub end: u32,
    pub book: String,
    pub chapter: u32,
    pub first_verse: u32,
    pub last_verse: u32,
    /// pieces of one quotation that run over several lines (or fit several chapters) share a span number
    pub span: u32,
}

type VerseRef = (String, u32, u32);

/// {verse: every word of it in the manuscripts, as skeletons, per manuscript in reading order}
fn verse_sequences(lines: &[Line], segments: &[Segment]) -> BTreeMap<VerseRef, BTreeMap<u32, Vec<String>>> {
    let mut out: BTreeMap<VerseRef, BTreeMap<u32, Vec<String>>> = BTreeMap::new();
    for s in segments {
        let line = &lines[s.line as usize];
        let words = out.entry((s.book.clone(), s.chapter, s.verse)).or_default().entry(line.scroll).or_default();
        words.extend(line.words[s.start as usize..s.end as usize].iter().map(|w| skeleton(w)));
    }
    out
}

fn gram_key(words: &[String]) -> String {
    words.join(" ")
}

/// Map every run of `n` consecutive words (at least `min_chars` letters in all) to the verses it occurs in.
fn ngram_index(
    sequences: &BTreeMap<VerseRef, BTreeMap<u32, Vec<String>>>,
    n: usize,
    min_chars: usize,
) -> (HashMap<String, Vec<u32>>, Vec<VerseRef>) {
    let mut refs: Vec<VerseRef> = Vec::new();
    let mut index: HashMap<String, Vec<u32>> = HashMap::new();
    for (verse, by_scroll) in sequences {
        let id = refs.len() as u32;
        refs.push(verse.clone());
        for words in by_scroll.values() {
            if words.len() < n {
                continue;
            }
            for i in 0..=words.len() - n {
                let gram = &words[i..i + n];
                if gram.iter().all(|w| !w.is_empty()) && gram.iter().map(|w| w.chars().count()).sum::<usize>() >= min_chars {
                    let entry = index.entry(gram_key(gram)).or_default();
                    if !entry.contains(&id) {
                        entry.push(id);
                    }
                }
            }
        }
    }
    (index, refs)
}

/// Find stretches of non-biblical text that match biblical text for at least `n` words in a row.
///
/// A match that fits more than `max_verses` different verses is thrown away as a formula (for
/// example "the LORD your God") that tells us nothing about which verse is meant, and a match
/// that fits several verses must be at least `ambiguous_min_words` long.
pub fn find_quotes(
    lines: &[Line],
    segments: &[Segment],
    n: usize,
    min_chars: usize,
    max_verses: usize,
    ambiguous_min_words: usize,
) -> Vec<Quote> {
    let (index, refs) = ngram_index(&verse_sequences(lines, segments), n, min_chars);
    let mut quotes = Vec::new();
    let mut span_number = 0u32;
    let mut i = 0;
    while i < lines.len() {
        // one fragment at a time, so that a quotation can continue onto the next line
        let mut j = i;
        while j + 1 < lines.len() && lines[j + 1].scroll == lines[i].scroll && lines[j + 1].frag == lines[i].frag {
            j += 1;
        }
        if !lines[i].biblical {
            // (skeleton, line, position)
            let mut flat: Vec<(String, usize, usize)> = Vec::new();
            for (li, line) in lines.iter().enumerate().take(j + 1).skip(i) {
                if line.biblical {
                    continue;
                }
                for (pos, word) in line.words.iter().enumerate() {
                    flat.push((skeleton(word), li, pos));
                }
            }
            // [first, end, votes per verse]
            let mut spans: Vec<(usize, usize, HashMap<u32, u32>)> = Vec::new();
            if flat.len() >= n {
                for k in 0..=flat.len() - n {
                    let gram: Vec<String> = flat[k..k + n].iter().map(|f| f.0.clone()).collect();
                    if gram.iter().any(|w| w.is_empty()) {
                        continue;
                    }
                    let Some(verses) = index.get(&gram_key(&gram)) else { continue };
                    match spans.last_mut() {
                        Some(last) if k <= last.1 => {
                            last.1 = k + n;
                            for &v in verses {
                                *last.2.entry(v).or_insert(0) += 1;
                            }
                        }
                        _ => spans.push((k, k + n, verses.iter().map(|&v| (v, 1)).collect())),
                    }
                }
            }
            for (first, end, votes) in spans {
                let top = votes.values().copied().max().unwrap_or(0) as f64;
                let mut chosen: Vec<&VerseRef> =
                    votes.iter().filter(|(_, &c)| c as f64 >= top * 0.5).map(|(&v, _)| &refs[v as usize]).collect();
                chosen.sort();
                if chosen.len() > max_verses || (chosen.len() > 1 && end - first < ambiguous_min_words) {
                    continue;
                }
                span_number += 1;
                // per line, per chapter
                let mut runs: Vec<(usize, usize, usize)> = Vec::new();
                for item in &flat[first..end] {
                    let (_, li, pos) = item;
                    match runs.last_mut() {
                        Some(run) if run.0 == *li && run.2 == *pos => run.2 = pos + 1,
                        _ => runs.push((*li, *pos, pos + 1)),
                    }
                }
                let mut chapters: Vec<((String, u32), Vec<u32>)> = Vec::new();
                for (book, chapter, verse) in &chosen {
                    match chapters.iter_mut().find(|(key, _)| key.0 == *book && key.1 == *chapter) {
                        Some((_, verses)) => verses.push(*verse),
                        None => chapters.push(((book.clone(), *chapter), vec![*verse])),
                    }
                }
                for (li, s, e) in runs {
                    for ((book, chapter), verses) in &chapters {
                        quotes.push(Quote {
                            line: li as u32,
                            start: s as u32,
                            end: e as u32,
                            book: book.clone(),
                            chapter: *chapter,
                            first_verse: *verses.iter().min().unwrap(),
                            last_verse: *verses.iter().max().unwrap(),
                            span: span_number,
                        });
                    }
                }
            }
        }
        i = j + 1;
    }
    quotes
}

/// The contents of `data/bible.json`.
pub fn build_bible(lines: &[Line], segments: &[Segment]) -> Json {
    let quotes = find_quotes(lines, segments, 4, 9, 6, 6);
    let present: HashSet<&str> = segments.iter().map(|s| s.book.as_str()).chain(quotes.iter().map(|q| q.book.as_str())).collect();
    let books: Vec<(&str, &str)> = BOOKS.iter().copied().filter(|(code, _)| present.contains(code)).collect();
    let index: HashMap<&str, usize> = books.iter().enumerate().map(|(i, (code, _))| (*code, i)).collect();

    let mut meta = Map::new();
    meta.insert(
        "note".into(),
        json!(
            "Verse segments come from the dataset's own book/chapter/verse labels. Quotes are found automatically by matching four or more consecutive words and are a guide, not an edition."
        ),
    );
    meta.insert("segments".into(), json!(segments.len()));
    meta.insert("quotes".into(), json!(quotes.len()));

    let mut out = Map::new();
    out.insert("meta".into(), Json::Object(meta));
    out.insert("books".into(), Json::Array(books.iter().map(|(code, name)| json!([code, name])).collect()));
    out.insert(
        "segments".into(),
        Json::Array(segments.iter().map(|s| json!([s.line, index[s.book.as_str()], s.chapter, s.verse, s.start, s.end])).collect()),
    );
    out.insert(
        "quotes".into(),
        Json::Array(
            quotes
                .iter()
                .map(|q| json!([q.line, q.start, q.end, index[q.book.as_str()], q.chapter, q.first_verse, q.last_verse, q.span]))
                .collect(),
        ),
    );
    Json::Object(out)
}

/// Read the verse segments back out of a `bible.json` (to check or redo the quotation search).
pub fn segments_from_json(bible: &Json) -> Result<Vec<Segment>, String> {
    let books: Vec<String> =
        bible["books"].as_array().ok_or("bible.json has no books")?.iter().map(|b| b[0].as_str().unwrap_or("").to_string()).collect();
    let number = |v: &Json| v.as_u64().map(|n| n as u32).ok_or_else(|| "bible.json: a segment has a bad number".to_string());
    bible["segments"]
        .as_array()
        .ok_or("bible.json has no segments")?
        .iter()
        .map(|s| {
            Ok(Segment {
                line: number(&s[0])?,
                book: books.get(number(&s[1])? as usize).cloned().ok_or("bible.json: a segment names an unknown book")?,
                chapter: number(&s[2])?,
                verse: number(&s[3])?,
                start: number(&s[4])?,
                end: number(&s[5])?,
            })
        })
        .collect()
}
