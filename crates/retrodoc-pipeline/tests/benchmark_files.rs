//! The committed files of `benchmark/`: a typo in a reference or a pair would only show up in a paid run.

use std::path::{Path, PathBuf};

use regex::RegexBuilder;
use retrodoc_pipeline::benchmark::{matching::score, Matches, Reference};
use serde::Deserialize;

/// The fields of `entry-points.yaml` that `examples/entry_points_replay.rs` reads.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryPointReference {
    repo: String,
    files: Vec<String>,
    entry_points: Vec<Item>,
    actors: Vec<Actor>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    label: String,
    matches: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Actor {
    name: String,
    matches: String,
}

fn benchmark_dirs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn name(dir: &Path) -> String {
    dir.file_name().unwrap().to_string_lossy().into_owned()
}

#[test]
fn the_benchmark_holds_two_applications_besides_the_smoke_repository() {
    let names: Vec<String> = benchmark_dirs().iter().map(|d| name(d)).collect();
    for wanted in ["delivery_router", "linkding", "lobsters"] {
        assert!(
            names.iter().any(|n| n == wanted),
            "{wanted} is missing from {names:?}"
        );
    }
}

#[test]
fn every_reference_loads_and_is_pinned_to_a_public_commit() {
    for dir in benchmark_dirs() {
        let reference = Reference::load(&dir.join("reference.yaml")).unwrap();
        assert!(
            reference.repository.starts_with("https://github.com/"),
            "{}: {}",
            name(&dir),
            reference.repository
        );
        assert!(
            reference.commit.len() == 40 && reference.commit.chars().all(|c| c.is_ascii_hexdigit()),
            "{}: the commit must be a full hash, got {}",
            name(&dir),
            reference.commit
        );
    }
}

#[test]
fn every_hand_written_pair_names_an_item_of_its_reference() {
    for dir in benchmark_dirs() {
        let reference = Reference::load(&dir.join("reference.yaml")).unwrap();
        let path = dir.join("matches.yaml");
        // `Matches::load` takes a missing file or a misspelled key for no pair at all.
        let raw = std::fs::read_to_string(&path).unwrap();
        let keys: serde_yaml::Mapping = serde_yaml::from_str(&raw).unwrap();
        for key in keys.keys() {
            assert!(
                matches!(key.as_str(), Some("domains" | "features")),
                "{}: unknown key {key:?}",
                name(&dir)
            );
        }
        let matches = Matches::load(&path).unwrap();
        let domains: Vec<String> = reference.domains.iter().map(|d| d.name.clone()).collect();
        let features: Vec<String> = reference
            .domains
            .iter()
            .flat_map(|d| d.features.iter().map(|f| f.name.clone()))
            .collect();
        score(&[], &domains, &matches.domains, &path).unwrap();
        score(&[], &features, &matches.features, &path).unwrap();
    }
}

#[test]
fn every_entry_points_reference_parses_and_names_its_repository() {
    for dir in benchmark_dirs() {
        let raw = std::fs::read_to_string(dir.join("entry-points.yaml")).unwrap();
        let entry_points: EntryPointReference = serde_yaml::from_str(&raw).unwrap();
        assert_eq!(entry_points.repo, name(&dir));
        assert!(!entry_points.files.is_empty(), "{}", name(&dir));
        assert!(!entry_points.entry_points.is_empty(), "{}", name(&dir));
        assert!(!entry_points.actors.is_empty(), "{}", name(&dir));
        for item in &entry_points.entry_points {
            assert!(
                !item.label.is_empty(),
                "{}: an entry point has no label",
                name(&dir)
            );
            compiles(&dir, &item.matches);
        }
        for actor in &entry_points.actors {
            assert!(
                !actor.name.is_empty(),
                "{}: an actor has no name",
                name(&dir)
            );
            compiles(&dir, &actor.matches);
        }
    }
}

/// `entry_points_replay` builds the same regex and panics on a bad one.
fn compiles(dir: &Path, pattern: &str) {
    assert!(!pattern.is_empty(), "{}: empty pattern", name(dir));
    if let Err(error) = RegexBuilder::new(pattern).case_insensitive(true).build() {
        panic!("{}: bad regex {pattern}: {error}", name(dir));
    }
}
