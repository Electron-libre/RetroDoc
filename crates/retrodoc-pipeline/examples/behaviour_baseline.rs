//! Measures the old path (domains from the files) on saved runs, with no LLM call
//! (`issues/done/behaviour_coverage_domains.md`, step 1): how many source files a behaviour reaches, and what the
//! last `generate` cost per pass.
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example behaviour_baseline -- <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache/` (the `entry-points.yaml`,
//! `glossary.yaml`, `roles.yaml` and `usage.json` of a `generate`). A file is *reached* when it holds an
//! entry point, is a home or mention file of an entity, or is in the slice (depth 2, 8 files, as the use
//! cases pass cuts it) of the file of an entry point. One row per run, Markdown.

#![allow(clippy::cast_precision_loss)] // counts of files and milliseconds: far below 2^52

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::{FileEntry, FileKind};
use retrodoc_pipeline::usage_log::{load_history, RunUsage};
use retrodoc_pipeline::{CodeIndex, EntryPoints, Glossary, RoleRules};

/// As in the use cases pass.
const SLICE_DEPTH: usize = 2;
const MAX_SLICE_FILES: usize = 8;

/// The files a behaviour reaches among `sources`.
fn reached(
    repo_root: &Path,
    sources: &[PathBuf],
    entry_points: &EntryPoints,
    glossary: &Glossary,
) -> BTreeSet<PathBuf> {
    let known: BTreeSet<&PathBuf> = sources.iter().collect();
    let index = CodeIndex::new(sources.iter().map(PathBuf::as_path));
    let mut reached: BTreeSet<PathBuf> = BTreeSet::new();
    for (file, entries) in &entry_points.files {
        if entries.entry_points.is_empty() {
            continue;
        }
        reached.insert(file.clone());
        reached.extend(index.slice(
            repo_root,
            std::slice::from_ref(file),
            SLICE_DEPTH,
            MAX_SLICE_FILES,
        ));
    }
    for entity in glossary.merged_entities() {
        reached.extend(entity.files);
    }
    reached.retain(|f| known.contains(f));
    reached
}

/// The last `generate` of a history, if any.
fn last_generate(history: Vec<RunUsage>) -> Option<RunUsage> {
    history.into_iter().rev().find(|r| r.command == "generate")
}

/// `(calls, wall ms)` of a pass in a run; `(0, 0)` when it did not run.
fn pass_cost(run: &RunUsage, pass: &str) -> (u64, u64) {
    run.passes
        .iter()
        .find(|p| p.name == pass)
        .map_or((0, 0), |p| {
            (p.models.iter().map(|m| m.calls).sum(), p.wall_ms)
        })
}

fn minutes(ms: u64) -> String {
    format!("{:.1}", ms as f64 / 60_000.0)
}

fn sources_of(repo_root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<FileEntry> = walk_repo(repo_root, &[]).expect("tree is walkable");
    if let Some(rules) = RoleRules::load(repo_root) {
        rules.promote_sources(&mut files);
    }
    files
        .into_iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path)
        .collect()
}

fn row(repo_root: &Path) -> Option<String> {
    let entry_points = EntryPoints::load(repo_root)?;
    let glossary = Glossary::load(repo_root).unwrap_or_default();
    let sources = sources_of(repo_root);
    let reached = reached(repo_root, &sources, &entry_points, &glossary);
    let share = 100.0 * reached.len() as f64 / sources.len().max(1) as f64;
    let mut line = format!(
        "| {} | {} | {} | {share:.0}% |",
        repo_root.file_name()?.to_string_lossy(),
        sources.len(),
        reached.len()
    );
    match last_generate(load_history(repo_root)) {
        Some(run) => {
            let calls: u64 = run
                .passes
                .iter()
                .flat_map(|p| &p.models)
                .map(|m| m.calls)
                .sum();
            let (map_calls, map_ms) = pass_cost(&run, "repo-map");
            let (dom_calls, dom_ms) = pass_cost(&run, "domains");
            let _ = write!(
                line,
                " {calls} | {} | {map_calls} ({} min) | {dom_calls} ({} min) |",
                minutes(run.wall_ms),
                minutes(map_ms),
                minutes(dom_ms)
            );
        }
        None => line += " - | - | - | - |",
    }
    Some(line)
}

fn main() {
    let dirs: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if dirs.is_empty() {
        eprintln!("usage: behaviour_baseline <run dir>...");
        std::process::exit(2);
    }
    println!("| Run | Source files | Reached | Share | Calls | Minutes | Repo map | Domains |");
    println!("|---|---|---|---|---|---|---|---|");
    for dir in dirs {
        match row(&dir) {
            Some(line) => println!("{line}"),
            None => eprintln!("{}: no entry-points.yaml, skipped", dir.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrodoc_pipeline::usage_log::{ModelRecord, PassRecord};

    fn run(command: &str, passes: Vec<PassRecord>) -> RunUsage {
        RunUsage {
            command: command.to_string(),
            finished_at: String::new(),
            wall_ms: 60_000,
            passes,
        }
    }

    fn pass(name: &str, calls: u64, wall_ms: u64) -> PassRecord {
        PassRecord {
            name: name.to_string(),
            wall_ms,
            models: vec![ModelRecord {
                model: "m".to_string(),
                calls,
                calls_without_usage: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
            }],
            unparseable: 0,
            skipped: 0,
        }
    }

    #[test]
    fn the_last_generate_is_the_one_measured_and_a_missing_pass_costs_nothing() {
        let history = vec![
            run("generate", vec![pass("repo-map", 9, 1)]),
            run("generate", vec![pass("repo-map", 5, 120_000)]),
            run("benchmark", vec![pass("repo-map", 1, 1)]),
        ];
        let last = last_generate(history).unwrap();
        assert_eq!(pass_cost(&last, "repo-map"), (5, 120_000));
        assert_eq!(pass_cost(&last, "domains"), (0, 0));
        assert!(last_generate(vec![run("benchmark", vec![])]).is_none());
    }

    #[test]
    fn an_entry_point_file_its_slice_and_the_entity_files_are_reached_the_rest_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, text: &str| std::fs::write(dir.path().join(name), text).unwrap();
        write("router.rb", "class Router; def run; Order.new; end; end");
        write("order.rb", "class Order; end");
        write("customer.rb", "class Customer; end");
        write("helper.rb", "module Helper; end");
        let sources: Vec<PathBuf> = ["router.rb", "order.rb", "customer.rb", "helper.rb"]
            .map(PathBuf::from)
            .into();
        let entry_points: EntryPoints = serde_yaml::from_str(
            "files:\n  router.rb:\n    content_hash: h\n    entry_points:\n    - {kind: public_api, name: run}\n  helper.rb:\n    content_hash: h\n    entry_points: []\n",
        )
        .unwrap();
        let glossary: Glossary = serde_yaml::from_str(
            "models:\n  customer.rb:\n    content_hash: h\n    entities:\n    - {name: Customer}\ntests: []\n",
        )
        .unwrap();

        let files = reached(dir.path(), &sources, &entry_points, &glossary);

        let names: Vec<_> = files
            .iter()
            .map(|f| f.to_string_lossy().into_owned())
            .collect();
        // The router, the order it names, and the customer entity; not the helper (no entry point in it).
        assert_eq!(names, ["customer.rb", "order.rb", "router.rb"]);
    }
}
