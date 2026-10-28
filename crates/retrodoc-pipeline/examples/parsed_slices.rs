//! Compares, with no LLM call, the slices of the entry points of saved runs built three ways
//! (`issues/parsed_references_for_ranking_and_slices.md`, step 1): the lexical follow of today, the parsed calls
//! with the lexical follow for what they do not resolve (what the use cases pass would use), and the parsed calls
//! alone (what the parser sees).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example parsed_slices -- [--detail] <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache/entry-points.yaml`. The slice of an
//! entry-point file is cut with the depth and budget of the use cases pass (depth 2, 8 files), from that file
//! alone (the pass starts from all the files of a feature), as `behaviour_baseline` does. One row per run, Markdown; with
//! `--detail`, the files each way adds or loses, per entry-point file whose slices differ.

#![allow(clippy::cast_precision_loss)] // counts of files: far below 2^52

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::{structure, FileEntry, FileKind};
use retrodoc_pipeline::{CodeIndex, EntryPoints, Follow, RoleRules};

/// As in the use cases pass.
const SLICE_DEPTH: usize = 2;
const MAX_SLICE_FILES: usize = 8;

/// What two slices of one file share and what each has alone.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Diff {
    common: Vec<PathBuf>,
    only_before: Vec<PathBuf>,
    only_after: Vec<PathBuf>,
}

fn diff(before: &[PathBuf], after: &[PathBuf]) -> Diff {
    let before: BTreeSet<&PathBuf> = before.iter().collect();
    let after: BTreeSet<&PathBuf> = after.iter().collect();
    let owned = |set: BTreeSet<&&PathBuf>| set.into_iter().map(|p| (*p).clone()).collect();
    Diff {
        common: owned(before.intersection(&after).collect()),
        only_before: owned(before.difference(&after).collect()),
        only_after: owned(after.difference(&before).collect()),
    }
}

/// Sums over the entry-point files of a run.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Totals {
    files: usize,
    parsed_files: usize,
    lexical: usize,
    mixed: usize,
    parsed: usize,
    /// Lexical vs mixed.
    mixed_diff: (usize, usize, usize),
    /// Lexical vs parsed alone.
    parsed_diff: (usize, usize, usize),
    /// Files whose mixed slice is the lexical one.
    mixed_same: usize,
}

impl Totals {
    fn add(&mut self, parsable: bool, lexical: &[PathBuf], mixed: &[PathBuf], parsed: &[PathBuf]) {
        let sum = |acc: &mut (usize, usize, usize), d: &Diff| {
            acc.0 += d.common.len();
            acc.1 += d.only_before.len();
            acc.2 += d.only_after.len();
        };
        self.files += 1;
        self.parsed_files += usize::from(parsable);
        self.lexical += lexical.len();
        self.mixed += mixed.len();
        self.parsed += parsed.len();
        let to_mixed = diff(lexical, mixed);
        self.mixed_same +=
            usize::from(to_mixed.only_before.is_empty() && to_mixed.only_after.is_empty());
        sum(&mut self.mixed_diff, &to_mixed);
        sum(&mut self.parsed_diff, &diff(lexical, parsed));
    }

    fn row(&self, run: &str) -> String {
        let mean = |n: usize| n as f64 / self.files.max(1) as f64;
        let (mc, ml, mm) = self.mixed_diff;
        let (pc, pl, pp) = self.parsed_diff;
        format!(
            "| {run} | {} | {} | {:.1} | {:.1} | {:.1} | {mc} / {ml} / {mm} | {} | {pc} / {pl} / {pp} |",
            self.files,
            self.parsed_files,
            mean(self.lexical),
            mean(self.mixed),
            mean(self.parsed),
            self.mixed_same,
        )
    }
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

fn list(files: &[PathBuf]) -> String {
    let names: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(", ")
    }
}

fn row(repo_root: &Path, detail: bool) -> Option<String> {
    let entry_points = EntryPoints::load(repo_root)?;
    let sources = sources_of(repo_root);
    let index = CodeIndex::new(sources.iter().map(PathBuf::as_path)).with_definitions(repo_root);
    let mut totals = Totals::default();
    let mut details = String::new();
    for (file, entries) in &entry_points.files {
        if entries.entry_points.is_empty() {
            continue;
        }
        let starts = std::slice::from_ref(file);
        let slice =
            |follow| index.slice_with(repo_root, starts, SLICE_DEPTH, MAX_SLICE_FILES, follow);
        let (lexical, mixed, parsed) = (
            slice(Follow::Lexical),
            slice(Follow::ParsedThenLexical),
            slice(Follow::Parsed),
        );
        totals.add(structure::is_supported(file), &lexical, &mixed, &parsed);
        let d = diff(&lexical, &mixed);
        if detail && !(d.only_before.is_empty() && d.only_after.is_empty()) {
            let _ = write!(
                details,
                "\n### {}\n- common: {}\n- lexical only: {}\n- mixed only: {}\n- parsed alone: {}\n",
                file.display(),
                list(&d.common),
                list(&d.only_before),
                list(&d.only_after),
                list(&parsed),
            );
        }
    }
    let run = repo_root.file_name()?.to_string_lossy().into_owned();
    Some(totals.row(&run) + &details)
}

fn main() {
    let mut detail = false;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--detail" {
            detail = true;
        } else {
            dirs.push(PathBuf::from(arg));
        }
    }
    if dirs.is_empty() {
        eprintln!("usage: parsed_slices [--detail] <run dir>...");
        std::process::exit(2);
    }
    println!("Mean slice size per entry-point file; common / lexical only / other only, summed over the files.\n");
    println!("| Run | Entry-point files | Parsable | Lexical | Mixed | Parsed | Lexical vs mixed | Mixed = lexical | Lexical vs parsed |");
    println!("|---|---|---|---|---|---|---|---|---|");
    for dir in dirs {
        match row(&dir, detail) {
            Some(line) => println!("{line}"),
            None => eprintln!("{}: no entry-points.yaml, skipped", dir.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_diff_splits_the_files_in_common_and_the_files_of_one_side() {
        let d = diff(&paths(&["a", "b", "c"]), &paths(&["c", "d", "a"]));
        assert_eq!(d.common, paths(&["a", "c"]));
        assert_eq!(d.only_before, paths(&["b"]));
        assert_eq!(d.only_after, paths(&["d"]));
    }

    #[test]
    fn totals_sum_the_files_and_count_the_unchanged_slices() {
        let mut totals = Totals::default();
        totals.add(
            true,
            &paths(&["a", "b"]),
            &paths(&["a", "b"]),
            &paths(&["c"]),
        );
        totals.add(false, &paths(&["a"]), &paths(&["a", "d"]), &[]);
        assert_eq!(totals.files, 2);
        assert_eq!(totals.parsed_files, 1);
        assert_eq!(totals.mixed_same, 1);
        assert_eq!(totals.mixed_diff, (3, 0, 1));
        assert_eq!(totals.parsed_diff, (0, 3, 1));
        assert_eq!(
            totals.row("r"),
            "| r | 2 | 1 | 1.5 | 2.0 | 0.5 | 3 / 0 / 1 | 1 | 0 / 3 / 1 |"
        );
    }
}
