//! The page (web/search.js) and the server have to agree on the settings, their defaults and
//! the columns of the answers. This needs Node (as the JavaScript tests do); without it the
//! test says so and passes.

use dss_search::corpus::Corpus;
use dss_search::query::{default_for, query, COMMON_DEFAULTS};
use dss_search::stats::{ANALYSES, COLLOC_SORTS, DIST_SORTS, KEYNESS_SORTS};
use serde_json::Value as Json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn page_facts() -> Option<Json> {
    let script = r#"
        const s = require('./web/search.js');
        const analyses = {};
        for (const a of Object.keys(s.statsAnalyses)) {
          analyses[a] = { defaults: s.statsDefaults(a), sorts: s.statsAnalyses[a].sorts.map(x => x[0]) };
        }
        console.log(JSON.stringify({ analyses, columns: Object.keys(s.statsColumns) }));
    "#;
    let output = Command::new("node").arg("-e").arg(script).current_dir(root()).output().ok()?;
    if !output.status.success() {
        panic!("node failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    serde_json::from_slice(&output.stdout).ok()
}

#[test]
fn the_page_and_the_server_agree() {
    let Some(page) = page_facts() else {
        println!("Node is not installed; skipping the check of web/search.js against the server.");
        return;
    };
    let analyses = page["analyses"].as_object().unwrap();
    let mut names: Vec<&str> = analyses.keys().map(|k| k.as_str()).collect();
    names.sort();
    let mut ours = ANALYSES.to_vec();
    ours.sort();
    assert_eq!(names, ours, "the page and the server offer different analyses");

    for (analysis, facts) in analyses {
        for (setting, value) in facts["defaults"].as_object().unwrap() {
            let page_value = match value {
                Json::String(s) => s.clone(),
                other => other.to_string(),
            };
            if page_value.is_empty() {
                continue; // text boxes start empty
            }
            let ours =
                COMMON_DEFAULTS.iter().find(|(name, _)| name == setting).map(|(_, d)| *d).unwrap_or_else(|| default_for(analysis, setting));
            assert_eq!(page_value, ours, "default of '{setting}' for {analysis}");
        }
        let sorts: Vec<&str> = facts["sorts"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
        let expected: &[&str] = match analysis.as_str() {
            "dist" => &DIST_SORTS,
            "colloc" => &COLLOC_SORTS,
            "keyness" => &KEYNESS_SORTS,
            _ => &[],
        };
        assert_eq!(sorts, expected, "sort choices of {analysis}");
    }

    // every column of every answer must be one the page knows how to show
    let known: Vec<&str> = page["columns"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    let dss = std::fs::read(root().join("data/dss.json")).unwrap();
    let corpus = Corpus::from_json(&dss, None).unwrap();
    for (analysis, extra) in [
        ("freq", vec![("top", "3")]),
        ("freq", vec![("unit", "form"), ("top", "3")]),
        ("dist", vec![("word", "יחד"), ("by", "scroll")]),
        ("dist", vec![("word", "יחד"), ("by", "cave")]),
        ("colloc", vec![("word", "יחד"), ("top", "3")]),
        ("colloc", vec![("word", "יחד"), ("unit", "form"), ("top", "3")]),
        ("keyness", vec![("target", "1QS"), ("top", "3")]),
        ("keyness", vec![("target", "1QS"), ("unit", "form"), ("top", "3")]),
    ] {
        let params: HashMap<String, String> = extra.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let answer = query(&corpus, analysis, &params).unwrap();
        for column in &answer.table.columns {
            assert!(known.contains(&column.as_str()), "the page does not know the column '{column}' of {analysis}");
        }
    }
}
