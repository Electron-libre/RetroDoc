//! Replays the saved runs of a benchmark repository against a hand-made reference of its entry points
//! and actors, with no LLM call (`issues/business_files_for_entry_points_and_actors.md`).
//!
//! Usage:
//! `cargo run -q -p retrodoc-pipeline --example entry_points_replay -- <reference.yaml> <tree> <dir>...`
//!
//! - `reference.yaml`: `benchmark/<repo>/entry-points.yaml`.
//! - `tree`: a checkout of the repository at the commit of the benchmark (the files the rules are applied to).
//! - `dir`: scanned recursively for `.retrodoc/cache` directories (one run each: `entry-points.yaml`,
//!   `actors.yaml`, `roles.yaml`, `business-files.yaml`, and the model of the `retrodoc.toml` above) and for
//!   loose `*.yaml` files that are a `roles.yaml` or a `business-files.yaml` (their file name must contain
//!   the repository name). A run belongs to the repository when its path contains the name or one of its
//!   files exists in the tree.
//!
//! Prints, per model, how much of the reference the entry-point role, the business list, their union and
//! the entry-points and actors passes reach: tables in Markdown.

#![allow(clippy::cast_precision_loss)] // counts of files and entries: far below 2^52

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use regex::{Regex, RegexBuilder};
use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::{FileEntry, FileKind};
use retrodoc_pipeline::{BusinessMap, FileRole, RoleRules};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Reference {
    repo: String,
    files: Vec<String>,
    entry_points: Vec<Item>,
    actors: Vec<Actor>,
}

#[derive(Debug, Deserialize)]
struct Item {
    label: String,
    matches: String,
}

#[derive(Debug, Deserialize)]
struct Actor {
    name: String,
    matches: String,
}

/// What one run saved.
#[derive(Debug, Default)]
struct Run {
    path: String,
    model: String,
    roles: Option<RoleRules>,
    business: Option<BusinessMap>,
    /// `file -> "<name> <verb> <resource>"` of its entry points; a file with none is still a key.
    entry_points: Option<BTreeMap<String, Vec<String>>>,
    actors: Option<Vec<String>>,
}

/// Share of the reference files a set of files holds, and how much of the set is outside the reference.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Coverage {
    reference_read: usize,
    reference_total: usize,
    read: usize,
}

impl Coverage {
    fn recall(self) -> f64 {
        ratio(self.reference_read, self.reference_total)
    }

    /// Share of the files read that are not in the reference.
    fn noise(self) -> f64 {
        1.0 - ratio(self.reference_read, self.read)
    }
}

fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        a as f64 / b as f64
    }
}

fn coverage(read: &BTreeSet<String>, reference: &[String]) -> Coverage {
    Coverage {
        reference_read: reference.iter().filter(|f| read.contains(*f)).count(),
        reference_total: reference.len(),
        read: read.len(),
    }
}

/// The source files `rules` classes as entry points in `tree`.
fn role_files(rules: &RoleRules, tree: &[FileEntry]) -> BTreeSet<String> {
    let mut files = tree.to_vec();
    rules.promote_sources(&mut files);
    let classified = rules.classify(&files);
    files
        .iter()
        .filter(|f| {
            f.kind == FileKind::Source
                && classified.roles.get(&f.path) == Some(&FileRole::EntryPoint)
        })
        .map(|f| f.path.to_string_lossy().into_owned())
        .collect()
}

fn business_files(map: &BusinessMap, tree: &[FileEntry]) -> BTreeSet<String> {
    map.files(tree)
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// `file -> texts` of an `entry-points.yaml`.
fn parse_entry_points(yaml: &str) -> Option<BTreeMap<String, Vec<String>>> {
    let doc: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    let files = doc.get("files")?.as_mapping()?;
    let text = |v: &serde_yaml::Value, key: &str| {
        v.get(key)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    Some(
        files
            .iter()
            .filter_map(|(path, file)| {
                let list = file.get("entry_points").and_then(|l| l.as_sequence());
                let texts = list.map_or_else(Vec::new, |l| {
                    l.iter()
                        .map(|e| {
                            format!(
                                "{} {} {}",
                                text(e, "name"),
                                text(e, "verb"),
                                text(e, "resource")
                            )
                        })
                        .collect()
                });
                Some((path.as_str()?.to_string(), texts))
            })
            .collect(),
    )
}

fn parse_actors(yaml: &str) -> Option<Vec<String>> {
    let doc: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    let list = doc.get("actors")?.as_sequence()?;
    Some(
        list.iter()
            .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(str::to_string))
            .collect(),
    )
}

fn regex(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .unwrap_or_else(|e| panic!("bad regex {pattern}: {e}"))
}

/// Reference groups with at least one entry point matching, and entry points outside the reference files.
fn entry_point_score(
    found: &BTreeMap<String, Vec<String>>,
    reference: &Reference,
) -> (usize, usize, usize) {
    let all: Vec<&String> = found.values().flatten().collect();
    let groups = reference
        .entry_points
        .iter()
        .filter(|g| {
            let re = regex(&g.matches);
            all.iter().any(|t| re.is_match(t))
        })
        .count();
    let in_reference: usize = found
        .iter()
        .filter(|(f, _)| reference.files.contains(f))
        .map(|(_, v)| v.len())
        .sum();
    (groups, in_reference, all.len())
}

/// Reference actors found, and the actors that match none.
fn actor_score(found: &[String], reference: &Reference) -> (usize, usize) {
    let res: Vec<Regex> = reference.actors.iter().map(|a| regex(&a.matches)).collect();
    let recalled = res
        .iter()
        .filter(|re| found.iter().any(|n| re.is_match(n)))
        .count();
    let extras = found
        .iter()
        .filter(|n| !res.iter().any(|re| re.is_match(n)))
        .count();
    (recalled, extras)
}

fn model_of(toml: &str) -> String {
    toml.lines()
        .find_map(|l| {
            let value = l.trim().strip_prefix("model")?.trim().strip_prefix('=')?;
            Some(value.trim().trim_matches('"').to_string())
        })
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn collect(dir: &Path, caches: &mut Vec<PathBuf>, loose: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.ends_with(".retrodoc/cache") {
                caches.push(path);
            } else if path
                .file_name()
                .is_none_or(|n| n != ".git" && n != "node_modules")
            {
                collect(&path, caches, loose);
            }
        } else if path.extension().is_some_and(|e| e == "yaml")
            && !path.to_string_lossy().contains(".retrodoc")
        {
            loose.push(path);
        }
    }
}

fn load_runs(dirs: &[PathBuf], reference: &Reference, tree: &[FileEntry]) -> Vec<Run> {
    let in_tree: BTreeSet<String> = tree
        .iter()
        .map(|f| f.path.to_string_lossy().into_owned())
        .collect();
    let (mut caches, mut loose) = (Vec::new(), Vec::new());
    for dir in dirs {
        collect(dir, &mut caches, &mut loose);
    }
    let mut runs = Vec::new();
    for cache in caches {
        let mut run = Run {
            path: cache.to_string_lossy().into_owned(),
            model: cache
                .parent()
                .and_then(Path::parent)
                .and_then(|root| read(&root.join("retrodoc.toml")))
                .map_or_else(|| "unknown".to_string(), |t| model_of(&t)),
            ..Run::default()
        };
        run.roles = read(&cache.join("roles.yaml")).and_then(|t| serde_yaml::from_str(&t).ok());
        run.business =
            read(&cache.join("business-files.yaml")).and_then(|t| serde_yaml::from_str(&t).ok());
        run.entry_points =
            read(&cache.join("entry-points.yaml")).and_then(|t| parse_entry_points(&t));
        run.actors = read(&cache.join("actors.yaml")).and_then(|t| parse_actors(&t));
        let known_paths: Vec<&String> = run
            .entry_points
            .iter()
            .flat_map(BTreeMap::keys)
            .chain(
                run.business
                    .iter()
                    .flat_map(|b| b.entries.iter().map(|e| &e.path)),
            )
            .collect();
        if run.path.contains(&reference.repo) || known_paths.iter().any(|p| in_tree.contains(*p)) {
            runs.push(run);
        }
    }
    for file in loose {
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !name.contains(&reference.repo) {
            continue;
        }
        let Some(text) = read(&file) else { continue };
        let mut run = Run {
            path: file.to_string_lossy().into_owned(),
            model: "unknown".to_string(),
            ..Run::default()
        };
        run.roles = serde_yaml::from_str::<RoleRules>(&text)
            .ok()
            .filter(|r| !r.rules.is_empty());
        run.business = serde_yaml::from_str::<BusinessMap>(&text)
            .ok()
            .filter(|b| !b.entries.is_empty());
        if run.roles.is_some() || run.business.is_some() {
            runs.push(run);
        }
    }
    runs
}

fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

/// `mean (min-max)` of a list of values, as percentages.
fn spread_pct(values: &[f64]) -> String {
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    format!("{} ({}-{})", pct(mean(values)), pct(min), pct(max))
}

fn spread_num(values: &[f64]) -> String {
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    format!("{:.1} ({min:.0}-{max:.0})", mean(values))
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len().max(1) as f64
}

fn report(runs: &[Run], reference: &Reference, tree: &[FileEntry]) -> String {
    let mut by_model: BTreeMap<&str, Vec<&Run>> = BTreeMap::new();
    for run in runs {
        by_model.entry(&run.model).or_default().push(run);
    }
    let mut out = format!(
        "# {}: {} reference files, {} entry-point groups, {} actors; {} runs\n",
        reference.repo,
        reference.files.len(),
        reference.entry_points.len(),
        reference.actors.len(),
        runs.len()
    );
    for (model, runs) in by_model {
        let _ = writeln!(out, "\n## {model}\n");
        out.push_str(&inputs_table(&runs, reference, tree));
        out.push_str(&entry_points_table(&runs, reference));
        out.push_str(&actors_table(&runs, reference));
        let missed = never_found(&runs, reference);
        if !missed.is_empty() {
            let _ = writeln!(out, "\nNever found by these runs: {}.", missed.join("; "));
        }
    }
    out
}

/// What the `entrypoint` role, the business list and both together would hand to the entry points pass.
fn inputs_table(runs: &[&Run], reference: &Reference, tree: &[FileEntry]) -> String {
    let mut out = String::from(
        "| Input | Samples | Reference files read | Files read | Outside the reference |\n|---|---|---|---|---|\n",
    );
    let sets: [(&str, Vec<BTreeSet<String>>); 3] = [
        (
            "`entrypoint` role",
            runs.iter()
                .filter_map(|r| r.roles.as_ref())
                .map(|x| role_files(x, tree))
                .collect(),
        ),
        (
            "business list",
            runs.iter()
                .filter_map(|r| r.business.as_ref())
                .map(|x| business_files(x, tree))
                .collect(),
        ),
        (
            "role + business list",
            runs.iter()
                .filter_map(|r| {
                    Some(
                        role_files(r.roles.as_ref()?, tree)
                            .union(&business_files(r.business.as_ref()?, tree))
                            .cloned()
                            .collect(),
                    )
                })
                .collect(),
        ),
    ];
    for (name, sets) in sets {
        if sets.is_empty() {
            continue;
        }
        let cov: Vec<Coverage> = sets.iter().map(|s| coverage(s, &reference.files)).collect();
        let recalls: Vec<f64> = cov.iter().map(|c| c.recall()).collect();
        let noises: Vec<f64> = cov.iter().map(|c| c.noise()).collect();
        let reads: Vec<f64> = cov.iter().map(|c| c.read as f64).collect();
        let _ = writeln!(
            out,
            "| {name} | {} | {} | {} | {} |",
            sets.len(),
            spread_pct(&recalls),
            spread_num(&reads),
            spread_pct(&noises)
        );
    }
    out
}

fn entry_points_table(runs: &[&Run], reference: &Reference) -> String {
    let with_eps: Vec<&Run> = runs
        .iter()
        .copied()
        .filter(|r| r.entry_points.is_some())
        .collect();
    if with_eps.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n| Entry-points pass | Runs | Entries | Groups found | Entries in reference files | Reference files read |\n|---|---|---|---|---|---|\n",
    );
    let (mut entries, mut groups, mut inref, mut files) = (vec![], vec![], vec![], vec![]);
    for run in &with_eps {
        let found = run.entry_points.as_ref().expect("filtered");
        let (g, i, n) = entry_point_score(found, reference);
        entries.push(n as f64);
        groups.push(ratio(g, reference.entry_points.len()));
        inref.push(ratio(i, n));
        let read: BTreeSet<String> = found.keys().cloned().collect();
        files.push(coverage(&read, &reference.files).recall());
    }
    let _ = writeln!(
        out,
        "| as run | {} | {} | {} | {} | {} |",
        with_eps.len(),
        spread_num(&entries),
        spread_pct(&groups),
        spread_pct(&inref),
        spread_pct(&files)
    );
    out
}

fn actors_table(runs: &[&Run], reference: &Reference) -> String {
    let with_actors: Vec<&Run> = runs
        .iter()
        .copied()
        .filter(|r| r.actors.is_some())
        .collect();
    if with_actors.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n| Actors pass | Runs | Actors | Reference actors found | Actors outside the reference |\n|---|---|---|---|---|\n",
    );
    let (mut counts, mut recalls, mut extras) = (vec![], vec![], vec![]);
    for run in &with_actors {
        let found = run.actors.as_ref().expect("filtered");
        let (r, e) = actor_score(found, reference);
        counts.push(found.len() as f64);
        recalls.push(ratio(r, reference.actors.len()));
        extras.push(e as f64);
    }
    let _ = writeln!(
        out,
        "| as run | {} | {} | {} | {} |",
        with_actors.len(),
        spread_num(&counts),
        spread_pct(&recalls),
        spread_num(&extras)
    );
    out
}

/// Labels of the reference entry-point groups and the names of the reference actors that no run found.
fn never_found(runs: &[&Run], reference: &Reference) -> Vec<String> {
    let texts: Vec<&String> = runs
        .iter()
        .filter_map(|r| r.entry_points.as_ref())
        .flat_map(|e| e.values().flatten())
        .collect();
    let names: Vec<&String> = runs
        .iter()
        .filter_map(|r| r.actors.as_ref())
        .flatten()
        .collect();
    let groups = reference
        .entry_points
        .iter()
        .filter(|g| !texts.is_empty() && !texts.iter().any(|t| regex(&g.matches).is_match(t)));
    let actors = reference
        .actors
        .iter()
        .filter(|a| !names.is_empty() && !names.iter().any(|n| regex(&a.matches).is_match(n)));
    groups
        .map(|g| g.label.clone())
        .chain(actors.map(|a| format!("actor {}", a.name)))
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: entry_points_replay <reference.yaml> <tree> <dir>...");
        std::process::exit(2);
    }
    let text = std::fs::read_to_string(&args[0]).expect("reference is readable");
    let reference: Reference = serde_yaml::from_str(&text).expect("reference is valid");
    let tree = walk_repo(Path::new(&args[1]), &[]).expect("tree is walkable");
    let dirs: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
    let runs = load_runs(&dirs, &reference, &tree);
    print!("{}", report(&runs, &reference, &tree));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> Reference {
        serde_yaml::from_str(
            "repo: r\nfiles: [a.rb, b.rb]\nentry_points:\n  - {label: add, matches: add_order}\n  - {label: clear, matches: clear_orders}\nactors:\n  - {name: customer, matches: customer}\n  - {name: rider, matches: rider}\n",
        )
        .unwrap()
    }

    #[test]
    fn coverage_counts_reference_files_read_and_noise() {
        let read: BTreeSet<String> = ["a.rb", "x.rb", "y.rb", "z.rb"].map(String::from).into();
        let c = coverage(&read, &reference().files);
        assert!((c.recall() - 0.5).abs() < 1e-9);
        assert!((c.noise() - 0.75).abs() < 1e-9);
        assert_eq!(coverage(&BTreeSet::new(), &reference().files).noise(), 1.0);
    }

    #[test]
    fn entry_points_are_read_per_file_and_scored_against_the_groups() {
        let yaml = "files:\n  a.rb:\n    content_hash: h\n    entry_points:\n    - {kind: public_api, name: 'Router#add_order', verb: create, resource: order}\n  c.rb:\n    content_hash: h\n    entry_points:\n    - {kind: other, name: Other, verb: do, resource: thing}\n  d.rb:\n    content_hash: h\n    entry_points: []\n";
        let found = parse_entry_points(yaml).unwrap();
        assert_eq!(
            found.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a.rb", "c.rb", "d.rb"]
        );
        assert_eq!(entry_point_score(&found, &reference()), (1, 1, 2));
    }

    #[test]
    fn actors_are_matched_by_regex_and_the_rest_counts_as_extra() {
        let found =
            parse_actors("actors:\n- name: Customer\n- name: Dispatcher\n- name: Delivery rider\n")
                .unwrap();
        assert_eq!(actor_score(&found, &reference()), (2, 1));
    }

    #[test]
    fn the_model_comes_from_the_toml() {
        assert_eq!(
            model_of("[llm]\nprovider = \"deepseek\"\nmodel = \"deepseek-chat\"\n"),
            "deepseek-chat"
        );
        assert_eq!(model_of("[ingest]\n"), "unknown");
    }
}
