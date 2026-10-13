//! Measures, with no LLM call, what leaving out of an application the `public_api` entry points that
//! no route, command, job, consumer or webhook reaches changes
//! (`issues/technical_and_back_office_use_cases.md`, step 5; `EntryPoints::without_unreached_public_api`).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example public_api_reach_measure -- <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache` (`entry-points.yaml`,
//! `use-cases.yaml`). Prints, per run, whether it reads as an application or a library (nothing is
//! left out of a library), the `public_api` entries left out, and the use cases that cite only
//! `public_api` entries, split by whether all of them are left out. Entries are matched by name
//! (the use cases cite names), so a name shared by two files counts once.

#![allow(clippy::cast_precision_loss)] // counts of entries: far below 2^52

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::FileKind;
use retrodoc_pipeline::{load_use_cases, EntryKind, EntryPoints, RoleRules};

fn sources_of(repo_root: &Path) -> Vec<PathBuf> {
    let mut files = walk_repo(repo_root, &[]).expect("tree is walkable");
    if let Some(rules) = RoleRules::load(repo_root) {
        rules.promote_sources(&mut files);
    }
    files
        .into_iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path)
        .collect()
}

fn percent(part: usize, whole: usize) -> String {
    format!("{:.0}%", 100.0 * part as f64 / whole.max(1) as f64)
}

fn report(repo_root: &Path) -> Option<String> {
    let entry_points = EntryPoints::load(repo_root)?;
    let use_cases = load_use_cases(repo_root)?;
    let sources = sources_of(repo_root);
    let api = entry_points
        .iter()
        .filter(|(_, e)| e.kind == EntryKind::PublicApi)
        .count();
    let application = entry_points.is_application();
    let kind_of: BTreeMap<String, EntryKind> = entry_points
        .iter()
        .map(|(_, e)| (e.name.clone(), e.kind))
        .collect();
    let (_, left_out) =
        entry_points.without_unreached_public_api(repo_root, sources.iter().map(PathBuf::as_path));
    let left_out: BTreeSet<&str> = left_out
        .iter()
        .filter_map(|id| id.split_once('#').map(|(_, name)| name))
        .collect();

    let (mut only_api, mut all_left_out) = (0, 0);
    for uc in &use_cases {
        let cited: Vec<&String> = uc
            .entry_points
            .iter()
            .filter(|n| kind_of.contains_key(*n))
            .collect();
        if cited.is_empty() || !cited.iter().all(|n| kind_of[*n] == EntryKind::PublicApi) {
            continue;
        }
        only_api += 1;
        all_left_out += usize::from(cited.iter().all(|n| left_out.contains(n.as_str())));
    }
    Some(format!(
        "## {}\n\n{}\n\n| | Count | Share |\n|---|---|---|\n\
         | `public_api` entries | {api} | |\n\
         | ... left out | {} | {} |\n\
         | use cases citing only `public_api` entries | {only_api} | {} of {} |\n\
         | ... all of them left out | {all_left_out} | |\n",
        repo_root.display(),
        if application {
            "application"
        } else {
            "library"
        },
        left_out.len(),
        percent(left_out.len(), api),
        percent(only_api, use_cases.len()),
        use_cases.len(),
    ))
}

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        eprintln!("usage: public_api_reach_measure <run dir>...");
        std::process::exit(2);
    }
    for dir in dirs {
        match report(Path::new(&dir)) {
            Some(text) => println!("{text}"),
            None => eprintln!("{dir}: no entry-points.yaml or use-cases.yaml"),
        }
    }
}
