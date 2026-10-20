//! Every source key `sim::tuning` cites (`&["SW-mds", ...]`) must be defined in `docs/research/`
//! (as `` `[SW-mds]` ``), so each number traces back to where it came from.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == extension))
        .collect();
    files.sort();
    files
}

/// The text between each `open` and the next `close`.
fn between<'a>(text: &'a str, open: &'a str, close: &'a str) -> impl Iterator<Item = &'a str> {
    text.split(open).skip(1).filter_map(move |rest| rest.split_once(close).map(|(inside, _)| inside))
}

fn defined_keys() -> BTreeSet<String> {
    files(&root().join("docs/research"), "md")
        .iter()
        .flat_map(|path| between(&std::fs::read_to_string(path).unwrap(), "`[", "]`").map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

fn cited_keys() -> BTreeSet<String> {
    files(&root().join("crates/sim/src/tuning"), "rs")
        .iter()
        .flat_map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            between(&text, "&[\"", "]")
                .flat_map(|list| list.split(',').map(|key| key.trim().trim_matches('"').to_owned()))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn every_cited_source_is_defined_in_the_research() {
    let defined = defined_keys();
    let cited = cited_keys();
    assert!(cited.len() >= 10, "found only {cited:?}; is the parser still reading tuning/?");
    let missing: Vec<&String> = cited.iter().filter(|key| !defined.contains(*key)).collect();
    assert!(missing.is_empty(), "cited in sim::tuning but not defined in docs/research/: {missing:?}");
}
