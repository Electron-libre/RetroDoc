//! Measures, with no LLM call, what giving a route to the context that owns its controller changes
//! (`issues/done/entry_points_of_a_large_context.md`, step 1).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example route_owner_measure -- <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache` (`roles.yaml`, `contexts.yaml`,
//! `entry-points.yaml`). Prints, per run, the routes (`http_route` entry points), how many move to another context or stay (no controller found, or several contexts own a
//! controller of that name), and the number of entry points per context before and after (the
//! contexts over the limit of one features prompt are marked).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::walker::walk_repo;
use retrodoc_ingest::FileKind;
use retrodoc_pipeline::{ContextMap, EntryKind, EntryOwners, EntryPoints, FileRole, RoleRules};

/// As `MAX_ENTRIES_PER_PROMPT` of the features pass.
const PROMPT_LIMIT: usize = 150;
const NONE: &str = "(file of no context)";

fn report(repo_root: &Path) -> Option<String> {
    let role_rules = RoleRules::load(repo_root)?;
    let mut files = walk_repo(repo_root, &[]).ok()?;
    role_rules.promote_sources(&mut files);
    let roles = role_rules.classify(&files);
    let sources: Vec<PathBuf> = files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path.clone())
        .collect();
    let seeds: Vec<PathBuf> = roles
        .files_with(FileRole::EntryPoint)
        .into_iter()
        .map(Path::to_path_buf)
        .collect();
    let resolved = ContextMap::load(repo_root)?.resolve(&sources, &seeds);
    let entry_points = EntryPoints::load(repo_root)?;
    let owners = EntryOwners::new(&resolved.contexts);

    let context_of_file: BTreeMap<&Path, &str> = resolved
        .contexts
        .iter()
        .flat_map(|c| c.files.iter().map(move |f| (f.as_path(), c.slug.as_str())))
        .collect();
    let mut before: BTreeMap<&str, usize> = BTreeMap::new();
    let mut after: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut routes, mut moved, mut in_place, mut no_owner, mut outside) = (0, 0, 0, 0, 0);
    for (file, entry) in entry_points.iter() {
        // A file of no context (the routes file is often one) is read for some context, which
        // then holds its entry points: `NONE` stands for it.
        let home = context_of_file.get(file).copied().unwrap_or(NONE);
        let target = owners.owner(file, entry);
        if entry.kind == EntryKind::HttpRoute {
            routes += 1;
            match target {
                Some(slug) if slug != home => moved += 1,
                Some(_) => in_place += 1,
                None => no_owner += 1,
            }
        }
        if home == NONE {
            outside += 1;
        }
        *before.entry(home).or_default() += 1;
        *after.entry(target.unwrap_or(home)).or_default() += 1;
    }

    let mut out = format!(
        "## {}\n\n{} entry points in {} contexts, {} in a file of no context.\n\
         Routes: {routes}; moved to another context: {moved}; \
         already in the right context: {in_place}; staying with their file (the route names a word of the file's name, or no single context owns the file it names): {no_owner}.\n\n| Context | Before | After |\n|---|---|---|\n",
        repo_root.file_name()?.to_string_lossy(),
        entry_points.iter().count(),
        resolved.contexts.len(),
        outside,
    );
    let mut rows: Vec<(&str, usize, usize)> = resolved
        .contexts
        .iter()
        .map(|c| c.slug.as_str())
        .chain(std::iter::once(NONE))
        .map(|slug| {
            (
                slug,
                before.get(slug).copied().unwrap_or(0),
                after.get(slug).copied().unwrap_or(0),
            )
        })
        .filter(|(_, b, a)| *b > 0 || *a > 0)
        .collect();
    rows.sort_by_key(|(_, b, a)| std::cmp::Reverse((*b).max(*a)));
    for (slug, b, a) in rows.iter().take(15) {
        let mark = |n: usize| {
            if n > PROMPT_LIMIT {
                format!("{n} (over {PROMPT_LIMIT})")
            } else {
                n.to_string()
            }
        };
        let _ = writeln!(out, "| {slug} | {} | {} |", mark(*b), mark(*a));
    }
    let over = |pick: fn(&(&str, usize, usize)) -> usize| {
        rows.iter().filter(|r| pick(r) > PROMPT_LIMIT).count()
    };
    let _ = writeln!(
        out,
        "\nContexts over {PROMPT_LIMIT}: {} before, {} after.",
        over(|r| r.1),
        over(|r| r.2)
    );
    Some(out)
}

fn main() {
    let dirs: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if dirs.is_empty() {
        eprintln!("usage: route_owner_measure <run dir>...");
        std::process::exit(2);
    }
    for dir in dirs {
        match report(&dir) {
            Some(text) => println!("{text}"),
            None => eprintln!("{}: roles, contexts or entry points missing", dir.display()),
        }
    }
}
