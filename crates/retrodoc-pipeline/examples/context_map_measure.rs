//! Measures, with no LLM call, whether a map of bounded contexts can be built before any code is read
//! (`issues/business_files_cap_on_large_application.md`, step 1): one context per entry point file name, the
//! files of the other layers joined by their name, then by the identifiers the context references.
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example context_map_measure -- [--names] <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache/roles.yaml`. A context is named
//! after the words of an entry point file's name, the words that repeat one of its directories left out
//! (`contracts_controller` under `controllers` gives `contract`). A source file joins the context whose
//! words start its own name or name one of its directories, the longest name first; the slices (depth 2,
//! 8 files, as the use cases pass cuts them) of the files of a context are then added. One row per run,
//! Markdown; `--names` also lists the largest contexts and the folders of the files in none (not for a confidential repository's committed files).

#![allow(clippy::cast_precision_loss)] // counts of files: far below 2^52

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::{FileEntry, FileKind};
use retrodoc_pipeline::{contexts_of, name_words, CodeIndex, FileRole, RoleRules};

/// As in the use cases pass.
const SLICE_DEPTH: usize = 2;
const MAX_SLICE_FILES: usize = 8;
/// Largest contexts listed by `--names`.
const NAMES_SHOWN: usize = 15;
/// Leading directories that name a folder in the list of the files in no context.
const FOLDER_DEPTH: usize = 2;

/// What one way of building the contexts gives.
struct Split {
    sizes: Vec<usize>,
    covered: usize,
    shared: usize,
}

impl Split {
    fn of(contexts: &BTreeMap<Vec<String>, BTreeSet<PathBuf>>) -> Self {
        let mut memberships: BTreeMap<&PathBuf, usize> = BTreeMap::new();
        for file in contexts.values().flatten() {
            *memberships.entry(file).or_insert(0) += 1;
        }
        let mut sizes: Vec<usize> = contexts.values().map(BTreeSet::len).collect();
        sizes.sort_unstable();
        Self {
            sizes,
            covered: memberships.len(),
            shared: memberships.values().filter(|n| **n > 1).count(),
        }
    }

    fn at(&self, percent: usize) -> usize {
        self.sizes
            .get((self.sizes.len().saturating_sub(1)) * percent / 100)
            .copied()
            .unwrap_or(0)
    }

    fn cells(&self, sources: usize) -> String {
        let share = |n: usize| 100.0 * n as f64 / sources.max(1) as f64;
        format!(
            " {:.0}% | {:.0}% | {} / {} / {} |",
            share(self.covered),
            share(self.shared),
            self.at(50),
            self.at(90),
            self.at(100)
        )
    }
}

fn sources_and_seeds(repo_root: &Path) -> Option<(Vec<PathBuf>, Vec<PathBuf>)> {
    let role_rules = RoleRules::load(repo_root)?;
    let mut files: Vec<FileEntry> = walk_repo(repo_root, &[]).expect("tree is walkable");
    role_rules.promote_sources(&mut files);
    let roles = role_rules.classify(&files);
    let sources: Vec<PathBuf> = files
        .into_iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path)
        .collect();
    let known: BTreeSet<&Path> = sources.iter().map(PathBuf::as_path).collect();
    let seeds = roles
        .files_with(FileRole::EntryPoint)
        .into_iter()
        .filter(|f| known.contains(f))
        .map(Path::to_path_buf)
        .collect();
    Some((sources, seeds))
}

/// The contexts by name alone, then with the slices of their files.
type Contexts = BTreeMap<Vec<String>, BTreeSet<PathBuf>>;

fn build(repo_root: &Path, sources: &[PathBuf], seeds: &[PathBuf]) -> (Contexts, Contexts) {
    let names: BTreeSet<Vec<String>> = seeds.iter().map(|f| name_words(f)).collect();
    let mut by_name: Contexts = names.iter().map(|n| (n.clone(), BTreeSet::new())).collect();
    for seed in seeds {
        by_name
            .entry(name_words(seed))
            .or_default()
            .insert(seed.clone());
    }
    for file in sources {
        for context in contexts_of(file, &names) {
            by_name.entry(context).or_default().insert(file.clone());
        }
    }
    let index = CodeIndex::new(sources.iter().map(PathBuf::as_path));
    let mut with_slices = by_name.clone();
    for files in with_slices.values_mut() {
        let starts: Vec<PathBuf> = files.iter().cloned().collect();
        for start in &starts {
            files.extend(index.slice(
                repo_root,
                std::slice::from_ref(start),
                SLICE_DEPTH,
                MAX_SLICE_FILES,
            ));
        }
    }
    (by_name, with_slices)
}

fn row(repo_root: &Path, show_names: bool) -> Option<String> {
    let (sources, seeds) = sources_and_seeds(repo_root)?;
    let (by_name, with_slices) = build(repo_root, &sources, &seeds);
    let mut line = format!(
        "| {} | {} | {} | {} |",
        repo_root.file_name()?.to_string_lossy(),
        sources.len(),
        seeds.len(),
        by_name.len()
    );
    line += &Split::of(&by_name).cells(sources.len());
    line += &Split::of(&with_slices).cells(sources.len());
    if show_names {
        let mut largest: Vec<(&Vec<String>, usize, usize)> = with_slices
            .iter()
            .map(|(name, files)| (name, by_name[name].len(), files.len()))
            .collect();
        largest.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(b.0)));
        for (name, named, sliced) in largest.into_iter().take(NAMES_SHOWN) {
            let _ = write!(
                line,
                "\n  {}: {named} by name, {sliced} with slices",
                name.join("_")
            );
        }
        let covered: BTreeSet<&PathBuf> = with_slices.values().flatten().collect();
        let mut outside: BTreeMap<PathBuf, usize> = BTreeMap::new();
        for file in sources.iter().filter(|f| !covered.contains(f)) {
            let folder: PathBuf = file.components().take(FOLDER_DEPTH).collect();
            *outside.entry(folder).or_insert(0) += 1;
        }
        let mut outside: Vec<(PathBuf, usize)> = outside.into_iter().collect();
        outside.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (folder, files) in outside.into_iter().take(NAMES_SHOWN) {
            let _ = write!(
                line,
                "\n  in no context: {files} under {}",
                folder.display()
            );
        }
    }
    Some(line)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let show_names = args.iter().any(|a| a == "--names");
    let dirs: Vec<&String> = args.iter().filter(|a| *a != "--names").collect();
    if dirs.is_empty() {
        eprintln!("usage: context_map_measure [--names] <run dir>...");
        std::process::exit(2);
    }
    println!(
        "| Run | Source files | Entry point files | Contexts | Covered (names) | In several | Size p50 / p90 / max | Covered (+ slices) | In several | Size p50 / p90 / max |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for dir in dirs {
        match row(Path::new(dir), show_names) {
            Some(line) => println!("{line}"),
            None => eprintln!("{dir}: no .retrodoc/cache/roles.yaml"),
        }
    }
}
