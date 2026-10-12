//! Measures, with no LLM call, how many of the files a context map leaves in the support can be placed
//! by their references (`issues/business_files_cap_on_large_application.md`, step 3b): a support file
//! joins the context whose files reference it or are referenced by it.
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example orphan_placement_measure -- [--names] <run dir>`
//!
//! A run dir is a checkout with `.retrodoc/cache/roles.yaml` and `contexts.yaml` (`retrodoc contexts`).
//! References are those of `CodeIndex::slice` at depth 1 (the files an identifier designates, 30 at most
//! per file). A support file has a score per context: the files of the context that reference it, plus
//! the files of the context it references. It is placed when the best score is at least `MIN_SCORE` and
//! at least twice the second one. `--names` also lists, per top folder, how many are placed.

#![allow(clippy::cast_precision_loss)] // counts of files: far below 2^52

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::{FileEntry, FileKind};
use retrodoc_pipeline::{CodeIndex, ContextMap, FileRole, RoleRules};

const MAX_REFS: usize = 30;
const MIN_SCORE: usize = 2;

/// The source files, the entry point files and the roles of the files.
type Repo = (Vec<PathBuf>, Vec<PathBuf>, BTreeMap<PathBuf, FileRole>);

fn sources_and_seeds(repo_root: &Path) -> Option<Repo> {
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
    Some((sources, seeds, roles.roles.clone()))
}

/// What the references say of one support file.
enum Verdict {
    None,
    Weak,
    Ambiguous,
    Placed(usize),
}

fn verdict(scores: &BTreeMap<usize, usize>) -> Verdict {
    let mut ranked: Vec<usize> = scores.values().copied().collect();
    ranked.sort_unstable_by(|a, b| b.cmp(a));
    let Some(&best) = ranked.first() else {
        return Verdict::None;
    };
    let second = ranked.get(1).copied().unwrap_or(0);
    if best < MIN_SCORE {
        Verdict::Weak
    } else if best < 2 * second {
        Verdict::Ambiguous
    } else {
        let context = scores.iter().find(|(_, s)| **s == best).map(|(c, _)| *c);
        Verdict::Placed(context.unwrap_or_default())
    }
}

fn run(repo_root: &Path, show_folders: bool) -> Option<String> {
    let (sources, seeds, roles) = sources_and_seeds(repo_root)?;
    let map = ContextMap::load(repo_root)?;
    let resolved = map.resolve(&sources, &seeds);
    let mut context_of: BTreeMap<&Path, usize> = BTreeMap::new();
    for (i, context) in resolved.contexts.iter().enumerate() {
        for file in &context.files {
            context_of.insert(file.as_path(), i);
        }
    }
    let index = CodeIndex::new(sources.iter().map(PathBuf::as_path));
    let refs: BTreeMap<&Path, Vec<PathBuf>> = sources
        .iter()
        .map(|file| {
            let starts = std::slice::from_ref(file);
            (file.as_path(), index.slice(repo_root, starts, 1, MAX_REFS))
        })
        .collect();

    // Incoming references: the support files each context file designates.
    let mut incoming: BTreeMap<&Path, BTreeMap<usize, usize>> = BTreeMap::new();
    for (file, designated) in &refs {
        if let Some(&context) = context_of.get(file) {
            for target in designated {
                *incoming
                    .entry(target.as_path())
                    .or_default()
                    .entry(context)
                    .or_insert(0) += 1;
            }
        }
    }

    let (mut none, mut weak, mut ambiguous, mut business) = (0, 0, 0, 0);
    let is_business = |file: &Path| {
        matches!(
            roles.get(file),
            Some(FileRole::EntryPoint | FileRole::Model | FileRole::Logic)
        )
    };
    let mut placed: BTreeMap<usize, usize> = BTreeMap::new();
    let mut by_folder: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut sample: Vec<String> = Vec::new();
    for file in &resolved.support {
        let mut scores = incoming.get(file.as_path()).cloned().unwrap_or_default();
        for target in refs.get(file.as_path()).into_iter().flatten() {
            if let Some(&context) = context_of.get(target.as_path()) {
                *scores.entry(context).or_insert(0) += 1;
            }
        }
        let folder: String = file
            .components()
            .take(2)
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let entry = by_folder.entry(folder).or_insert((0, 0));
        entry.0 += 1;
        match verdict(&scores) {
            Verdict::None => none += 1,
            Verdict::Weak => weak += 1,
            Verdict::Ambiguous => ambiguous += 1,
            Verdict::Placed(context) => {
                *placed.entry(context).or_insert(0) += 1;
                entry.1 += 1;
                business += usize::from(is_business(file));
                sample.push(format!(
                    "{} -> {}",
                    file.display(),
                    resolved.contexts[context].slug
                ));
            }
        }
    }
    let total = resolved.support.len();
    let placed_total: usize = placed.values().sum();
    let share = |n: usize| 100.0 * n as f64 / total.max(1) as f64;
    let mut out = format!(
        "| {} | {} | {} | {} ({:.0}%), {} of a business role | {} ({:.0}%) | {} ({:.0}%) | {} ({:.0}%) |",
        repo_root.file_name()?.to_string_lossy(),
        sources.len(),
        total,
        placed_total,
        share(placed_total),
        business,
        ambiguous,
        share(ambiguous),
        weak,
        share(weak),
        none,
        share(none),
    );
    if show_folders {
        for line in sample.iter().step_by(8) {
            let _ = write!(out, "\n  e.g. {line}");
        }
        let mut folders: Vec<(&String, &(usize, usize))> = by_folder.iter().collect();
        folders.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(b.0)));
        for (folder, (all, placed)) in folders.into_iter().take(20) {
            let _ = write!(out, "\n  {folder}: {placed} of {all} placed");
        }
    }
    Some(out)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let show_folders = args.iter().any(|a| a == "--names");
    let dirs: Vec<&String> = args.iter().filter(|a| *a != "--names").collect();
    if dirs.is_empty() {
        eprintln!("usage: orphan_placement_measure [--names] <run dir>...");
        std::process::exit(2);
    }
    println!("| Run | Source files | In the support | Placed | Ambiguous | Weak | No reference |");
    println!("|---|---|---|---|---|---|---|");
    for dir in dirs {
        match run(Path::new(dir), show_folders) {
            Some(line) => println!("{line}"),
            None => eprintln!("{dir}: no roles.yaml or contexts.yaml"),
        }
    }
}
