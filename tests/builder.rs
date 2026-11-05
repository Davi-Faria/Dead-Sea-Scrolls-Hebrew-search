//! Rebuilding the data from the ETCBC/dss dataset must give exactly the files that ship in data/.
//!
//! This needs the dataset's .tf files, so it only runs when DSS_TF_DIR points at them (for example
//! a clone of github.com/ETCBC/dss: DSS_TF_DIR=dss/tf/2.0.1). CI does this on every push.

use dss_search::builder::{build, DEFAULT_REF};
use dss_search::{bible, corpus::Corpus};
use serde_json::Value as Json;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn rebuilt_data_is_the_shipped_data() {
    let Ok(dir) = std::env::var("DSS_TF_DIR") else {
        println!("DSS_TF_DIR is not set; skipping the rebuild check.");
        return;
    };
    let built = build(&PathBuf::from(dir), DEFAULT_REF, &|_| {}).unwrap();
    let shipped: Json = serde_json::from_slice(&std::fs::read(root().join("data/dss.json")).unwrap()).unwrap();
    assert_eq!(built.data["meta"], shipped["meta"], "meta");
    assert_eq!(built.data["scrolls"], shipped["scrolls"], "scrolls");
    assert_eq!(built.data["lemmas"], shipped["lemmas"], "lemmas");
    assert_eq!(built.data["lemmaLang"], shipped["lemmaLang"], "lemmaLang");
    let (a, b) = (built.data["lines"].as_array().unwrap(), shipped["lines"].as_array().unwrap());
    assert_eq!(a.len(), b.len(), "number of lines");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert_eq!(x, y, "line {i}");
    }

    let rebuilt_bible = bible::build_bible(&built.lines, &built.segments);
    let shipped_bible: Json = serde_json::from_slice(&std::fs::read(root().join("data/bible.json")).unwrap()).unwrap();
    assert_eq!(rebuilt_bible, shipped_bible, "bible.json");

    // and the corpus made from it loads
    let corpus = Corpus::from_json(&serde_json::to_vec(&built.data).unwrap(), None).unwrap();
    assert_eq!(corpus.tokens.len(), shipped["meta"]["words"].as_u64().unwrap() as usize);
}
