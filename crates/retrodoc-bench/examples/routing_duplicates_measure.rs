//! Measures, with no LLM call, what leaving out the routes a routing file declares for other
//! files changes (`issues/done/routes_file_entries_per_route.md`,
//! `EntryPoints::without_routes_declared_elsewhere`).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example routing_duplicates_measure -- <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache` (`entry-points.yaml`, and
//! `features.yaml`, `use-cases.yaml` when the run got that far). Prints, per run, the entries left
//! out by file, then the features and the use cases that hold only entries left out (they cite the
//! ids `<file>#<name>`; a run saved with names counts none).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_pipeline::{load_features, load_use_cases, EntryPoints};

fn only_left_out<'a>(ids: &[String], left_out: &BTreeSet<&str>, slug: &'a str) -> Option<&'a str> {
    (!ids.is_empty() && ids.iter().all(|id| left_out.contains(id.as_str()))).then_some(slug)
}

fn report(repo_root: &Path) -> Option<String> {
    let entry_points = EntryPoints::load(repo_root)?;
    let total = entry_points.iter().count();
    let (_, left_out) = entry_points.without_routes_declared_elsewhere();
    let mut by_file: BTreeMap<&str, usize> = BTreeMap::new();
    for id in &left_out {
        *by_file
            .entry(id.split_once('#').map_or(id.as_str(), |(f, _)| f))
            .or_default() += 1;
    }
    let left_out: BTreeSet<&str> = left_out.iter().map(String::as_str).collect();
    let features = load_features(repo_root).unwrap_or_default();
    let use_cases = load_use_cases(repo_root).unwrap_or_default();
    let features_gone: Vec<&str> = features
        .iter()
        .filter_map(|f| only_left_out(&f.entry_points, &left_out, &f.slug))
        .collect();
    let use_cases_gone: Vec<&str> = use_cases
        .iter()
        .filter_map(|u| only_left_out(&u.entry_points, &left_out, &u.slug))
        .collect();
    let mut text = format!(
        "## {}\n\n{} of {total} entries left out\n",
        repo_root.display(),
        left_out.len()
    );
    for (file, count) in by_file {
        let _ = writeln!(text, "- `{file}`: {count}");
    }
    let _ = write!(
        text,
        "\nfeatures holding only entries left out: {} of {} {features_gone:?}\n\
         use cases citing only entries left out: {} of {} {use_cases_gone:?}\n",
        features_gone.len(),
        features.len(),
        use_cases_gone.len(),
        use_cases.len(),
    );
    Some(text)
}

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        eprintln!("usage: routing_duplicates_measure <run dir>...");
        std::process::exit(2);
    }
    for dir in dirs {
        match report(Path::new(&dir)) {
            Some(text) => println!("{text}"),
            None => eprintln!("{dir}: no entry-points.yaml"),
        }
    }
}
