//! The quotation search must reproduce the shipped data/bible.json from the shipped verse segments.

use dss_search::bible::{build_bible, segments_from_json};
use dss_search::corpus::Corpus;
use serde_json::Value as Json;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn quotations_match_the_shipped_bible_json() {
    let dss = std::fs::read(root().join("data/dss.json")).unwrap();
    let corpus = Corpus::from_json(&dss, None).unwrap();
    let shipped: Json = serde_json::from_slice(&std::fs::read(root().join("data/bible.json")).unwrap()).unwrap();
    let segments = segments_from_json(&shipped).unwrap();
    let rebuilt = build_bible(&corpus.lines, &segments);

    assert_eq!(rebuilt["meta"], shipped["meta"], "meta");
    assert_eq!(rebuilt["books"], shipped["books"], "books");
    assert_eq!(rebuilt["segments"], shipped["segments"], "segments");
    let (a, b) = (rebuilt["quotes"].as_array().unwrap(), shipped["quotes"].as_array().unwrap());
    assert_eq!(a.len(), b.len(), "number of quotations");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert_eq!(x, y, "quotation {i}");
    }
}
