//! A small reader for the Text-Fabric (`.tf`) files of the ETCBC/dss dataset.
//!
//! A `.tf` file is a header (lines starting with `@`), a blank line, and then one row per node:
//! `node<TAB>value`, where the node may be a range (`5-9`) or a list (`3,5-7`), or just
//! `value`, which belongs to the node after the previous one. Edge files (here only `oslots`)
//! have the same shape with a set of slots instead of a value. This reads exactly what
//! `builder.rs` needs and nothing more: features of the nodes in a wanted range, the node
//! types, and the slots of some nodes.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

fn open(path: &Path) -> Result<BufReader<File>, String> {
    File::open(path).map(BufReader::new).map_err(|e| format!("Could not open {}: {e}", path.display()))
}

/// `\\`, `\t` and `\n` in a value stand for a backslash, a tab and a newline.
fn unescape(value: &str) -> String {
    if !value.contains('\\') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// `1-5,7,9-12` as ranges.
fn parse_ranges(spec: &str, path: &Path) -> Result<Vec<(u32, u32)>, String> {
    let bad = || format!("{}: cannot read the node or slot list '{spec}'", path.display());
    let mut out = Vec::new();
    for part in spec.split(',').filter(|p| !p.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.parse::<u32>().map_err(|_| bad())?, b.parse::<u32>().map_err(|_| bad())?),
            None => {
                let n = part.parse::<u32>().map_err(|_| bad())?;
                (n, n)
            }
        };
        out.push((a, b));
    }
    Ok(out)
}

/// Sort ranges and join those that touch.
fn merge(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    ranges.sort_unstable();
    let mut out: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
    for (a, b) in ranges {
        match out.last_mut() {
            Some(last) if a <= last.1.saturating_add(1) => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// Go through the rows of a file. The callback gets the node ranges of the row (already counted
/// on from the previous row when the row names no node) and the raw text after the tab.
fn for_each_row(path: &Path, mut f: impl FnMut(&[(u32, u32)], &str) -> Result<(), String>) -> Result<(), String> {
    let mut reader = open(path)?;
    let mut line = String::new();
    let mut in_header = true;
    let mut previous: u32 = 0;
    loop {
        line.clear();
        let read = reader.read_line(&mut line).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        let text = line.strip_suffix('\n').unwrap_or(&line);
        let text = text.strip_suffix('\r').unwrap_or(text);
        if in_header {
            if text.is_empty() {
                in_header = false;
            }
            continue;
        }
        let (ranges, value) = match text.split_once('\t') {
            Some((spec, value)) => (parse_ranges(spec, path)?, value),
            None => (vec![(previous + 1, previous + 1)], text),
        };
        if let Some(last) = ranges.last() {
            previous = last.1;
        }
        f(&ranges, value)?;
    }
    Ok(())
}

/// The nodes of each type, as `(first node, last node, type)`.
pub fn read_otype(path: &Path) -> Result<Vec<(u32, u32, String)>, String> {
    let mut out = Vec::new();
    for_each_row(path, |ranges, value| {
        for &(a, b) in ranges {
            out.push((a, b, unescape(value)));
        }
        Ok(())
    })?;
    Ok(out)
}

/// The values of one feature for the nodes in a range, kept compactly (each distinct value once).
pub struct Feature {
    first: u32,
    /// 0 for no value, else index into `values` plus one
    ids: Vec<u32>,
    values: Vec<String>,
}

impl Feature {
    /// The value of a node, or None if it has none (an empty value counts as none).
    pub fn get(&self, node: u32) -> Option<&str> {
        let i = node.checked_sub(self.first)? as usize;
        match self.ids.get(i).copied().unwrap_or(0) {
            0 => None,
            id => Some(self.values[id as usize - 1].as_str()).filter(|v| !v.is_empty()),
        }
    }

    /// Does the node have a value at all (an empty one counts)?
    pub fn has(&self, node: u32) -> bool {
        node.checked_sub(self.first).and_then(|i| self.ids.get(i as usize)).is_some_and(|&id| id != 0)
    }
}

/// Read one feature, keeping only the nodes from `first` to `last`.
pub fn read_feature(path: &Path, first: u32, last: u32) -> Result<Feature, String> {
    let mut ids = vec![0u32; (last - first + 1) as usize];
    let mut values: Vec<String> = Vec::new();
    let mut known: HashMap<String, u32> = HashMap::new();
    for_each_row(path, |ranges, raw| {
        // skip rows outside the wanted nodes before doing any work on the value
        if ranges.iter().all(|&(a, b)| b < first || a > last) {
            return Ok(());
        }
        let value = unescape(raw);
        let id = *known.entry(value.clone()).or_insert_with(|| {
            values.push(value);
            values.len() as u32
        });
        for &(a, b) in ranges {
            for node in a.max(first)..=b.min(last) {
                ids[(node - first) as usize] = id;
            }
        }
        Ok(())
    })?;
    Ok(Feature { first, ids, values })
}

/// The slots of some nodes, as sorted, joined ranges.
pub struct Slots {
    first: u32,
    /// (offset into `ranges`, number of ranges) per node
    spans: Vec<(u32, u32)>,
    ranges: Vec<(u32, u32)>,
}

impl Slots {
    pub fn get(&self, node: u32) -> &[(u32, u32)] {
        match node.checked_sub(self.first).and_then(|i| self.spans.get(i as usize)) {
            Some(&(offset, len)) => &self.ranges[offset as usize..(offset + len) as usize],
            None => &[],
        }
    }
}

/// Read the `oslots` edges for the nodes from `first` to `last` for which `wanted` is true.
pub fn read_oslots(path: &Path, first: u32, last: u32, wanted: impl Fn(u32) -> bool) -> Result<Slots, String> {
    let mut spans = vec![(0u32, 0u32); (last - first + 1) as usize];
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for_each_row(path, |nodes, value| {
        if nodes.iter().all(|&(a, b)| b < first || a > last) {
            return Ok(());
        }
        let slots = merge(parse_ranges(value, path)?);
        for &(a, b) in nodes {
            for node in a.max(first)..=b.min(last) {
                if wanted(node) {
                    spans[(node - first) as usize] = (ranges.len() as u32, slots.len() as u32);
                    ranges.extend_from_slice(&slots);
                }
            }
        }
        Ok(())
    })?;
    Ok(Slots { first, spans, ranges })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp(name: &str, text: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("dss-tf-test-{}-{name}", std::process::id()));
        std::fs::File::create(&path).unwrap().write_all(text.as_bytes()).unwrap();
        path
    }

    #[test]
    fn reads_explicit_and_implicit_nodes() {
        let path = temp("feature.tf", "@node\n@valueType=str\n\n5\ta\nb\n\n9\t \n12-13\tz\n");
        let f = read_feature(&path, 1, 20).unwrap();
        assert_eq!(f.get(5), Some("a"));
        assert_eq!(f.get(6), Some("b"));
        assert_eq!(f.get(7), None); // a blank line is an empty value
        assert!(f.has(7));
        assert_eq!(f.get(9), Some(" ")); // a space is a value
        assert_eq!(f.get(8), None); // not mentioned
        assert!(!f.has(8));
        assert_eq!(f.get(12), Some("z")); // a range gives each node the value
        assert_eq!(f.get(13), Some("z"));
        assert_eq!(f.get(14), None);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn only_wanted_nodes_are_kept() {
        let path = temp("range.tf", "@node\n\n1\tx\ny\nz\n");
        let f = read_feature(&path, 2, 2).unwrap();
        assert_eq!(f.get(1), None);
        assert_eq!(f.get(2), Some("y"));
        assert_eq!(f.get(3), None);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn reads_types_and_slots() {
        let t = temp("otype.tf", "@node\n\n1-4\tsign\n5-6\tword\n");
        assert_eq!(read_otype(&t).unwrap(), vec![(1, 4, "sign".to_string()), (5, 6, "word".to_string())]);
        let o = temp("oslots.tf", "@edge\n\n5\t1-2,4\n3\n");
        let s = read_oslots(&o, 5, 6, |_| true).unwrap();
        assert_eq!(s.get(5), &[(1, 2), (4, 4)]);
        assert_eq!(s.get(6), &[(3, 3)]);
        assert_eq!(s.get(7), &[] as &[(u32, u32)]);
        std::fs::remove_file(t).ok();
        std::fs::remove_file(o).ok();
    }

    #[test]
    fn values_are_unescaped() {
        assert_eq!(unescape("a\\tb\\nc\\\\d"), "a\tb\nc\\d");
        assert_eq!(unescape("plain"), "plain");
    }
}
