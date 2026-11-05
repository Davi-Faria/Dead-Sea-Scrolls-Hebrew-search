//! Compares the answers of this program with answers made by the original Python/Polars version
//! (`tests/golden.json`, made by `tests/make_golden.py`). Set DSS_GOLDEN to use another file.

use dss_search::corpus::Corpus;
use dss_search::query::query;
use serde_json::Value as Json;
use std::collections::HashMap;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_corpus() -> Corpus {
    let dss = std::fs::read(root().join("data/dss.json")).unwrap();
    let names = std::fs::read(root().join("data/scroll-names.json")).ok();
    Corpus::from_json(&dss, names.as_deref()).unwrap()
}

/// Compare two JSON values; floats may differ by rounding in the last place.
fn same(a: &Json, b: &Json, path: &str, problems: &mut Vec<String>) {
    match (a, b) {
        (Json::Number(x), Json::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if (x - y).abs() > 1e-9 {
                problems.push(format!("{path}: {x} != {y}"));
            }
        }
        (Json::Array(x), Json::Array(y)) => {
            if x.len() != y.len() {
                problems.push(format!("{path}: length {} != {}", x.len(), y.len()));
                return;
            }
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                same(p, q, &format!("{path}[{i}]"), problems);
            }
        }
        (Json::Object(x), Json::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) => same(v, w, &format!("{path}.{k}"), problems),
                    None => problems.push(format!("{path}.{k}: missing in the Rust answer")),
                }
            }
            for k in y.keys() {
                if !x.contains_key(k) {
                    problems.push(format!("{path}.{k}: only in the Rust answer"));
                }
            }
        }
        _ => {
            if a != b {
                problems.push(format!("{path}: {a} != {b}"));
            }
        }
    }
}

#[test]
fn answers_match_the_python_version() {
    let file = std::env::var("DSS_GOLDEN").map(PathBuf::from).unwrap_or_else(|_| root().join("tests/golden.json"));
    let golden: Vec<Json> = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let corpus = load_corpus();
    let mut failures = 0;
    for case in &golden {
        let analysis = case["analysis"].as_str().unwrap();
        let params: HashMap<String, String> =
            case["params"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        let mut problems = Vec::new();
        match (case["ok"].as_bool().unwrap(), query(&corpus, analysis, &params)) {
            (true, Ok(answer)) => {
                let mut ours = answer.to_json();
                if case["truncated"].as_bool().unwrap_or(false) {
                    // the file keeps only the first rows of a long table
                    let kept = case["result"]["rows"].as_array().unwrap().len();
                    let rows = ours["rows"].as_array_mut().unwrap();
                    assert!(rows.len() > kept, "a table that was cut should be longer than what is kept");
                    rows.truncate(kept);
                }
                same(&case["result"], &ours, "", &mut problems)
            }
            (false, Err(error)) => {
                if error.0 != case["error"].as_str().unwrap() {
                    problems.push(format!("error text: {:?} != {:?}", case["error"], error.0));
                }
            }
            (true, Err(error)) => problems.push(format!("Rust failed: {error}")),
            (false, Ok(_)) => problems.push("Rust answered, Python raised an error".to_string()),
        }
        if !problems.is_empty() {
            failures += 1;
            println!("\n✗ {analysis} {}", case["params"]);
            for p in problems.iter().take(6) {
                println!("    {p}");
            }
            if problems.len() > 6 {
                println!("    ... and {} more", problems.len() - 6);
            }
        }
    }
    println!("\n{} of {} cases differ", failures, golden.len());
    assert_eq!(failures, 0);
}
