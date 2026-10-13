//! The slice of a run (`issues/business_files_cap_on_large_application.md`, step 4b): which contexts
//! a `generate` documents now, and which files it reads for them. No LLM call.
//!
//! The contexts are named by the user, else the next ones not done in the order of the map, as many
//! as fit a budget of files: a context is never cut, so at least one is taken. A context is done when
//! its whole chain finished (`mark_done`) and none of its files, nor of the files read for it,
//! changed since: their hash is kept in `fingerprints.json`, so an edited file makes it pending
//! again and a rerun of a done context is a no-op anyway. The order of the map is the order of the
//! runs: the first context that does not fit the budget ends the selection, even when a later one
//! would fit.
//!
//! For a context, the files read are those of its files that carry business (entry points, models,
//! logic) and the business files its entry points reference at depth 1 that no other context owns,
//! which is how a file no name reaches is read once a behaviour reaches it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, Fingerprints};
use crate::roles::{FileRole, RoleMap};
use crate::slices::CodeIndex;

use super::{ContextMap, Resolved, ResolvedContext};

/// Business files an entry point file references, at most, in a context.
const MAX_REFERENCED: usize = 200;

/// What a run documents.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunSlice {
    /// Slugs of the contexts of this run, in the order of the map.
    pub contexts: Vec<String>,
    /// The files to read for them, in path order.
    pub files: Vec<PathBuf>,
    /// Contexts neither done nor in this run.
    pub pending: Vec<String>,
    /// Contexts done (all their files unchanged since their chain finished).
    pub done: Vec<String>,
}

/// The hash of a context: its files, the files read for it, and their content.
fn context_hash(repo_root: &Path, context: &ResolvedContext, read: &[PathBuf]) -> String {
    let mut files: Vec<&PathBuf> = context.files.iter().chain(read).collect();
    files.sort();
    files.dedup();
    fingerprint(files.into_iter().map(|path| {
        let hash = std::fs::read(repo_root.join(path)).map_or_else(
            |_| "unreadable".to_string(),
            |bytes| hash_content(&String::from_utf8_lossy(&bytes)),
        );
        format!("{}\n{hash}", path.display())
    }))
}

/// What a plan needs about every context: the files read for each, found once.
struct Reading {
    /// Per context, in the order of the map.
    read: Vec<Vec<PathBuf>>,
}

impl Reading {
    fn of(repo_root: &Path, resolved: &Resolved, roles: &RoleMap, sources: &[PathBuf]) -> Self {
        let business = |file: &Path| {
            matches!(
                roles.roles.get(file),
                Some(FileRole::EntryPoint | FileRole::Model | FileRole::Logic)
            )
        };
        let code = CodeIndex::new(sources.iter().map(PathBuf::as_path));
        let mut owner: BTreeMap<&Path, usize> = BTreeMap::new();
        for (i, context) in resolved.contexts.iter().enumerate() {
            owner.extend(context.files.iter().map(|f| (f.as_path(), i)));
        }
        let read = resolved
            .contexts
            .iter()
            .enumerate()
            .map(|(index, context)| {
                let mut files: Vec<PathBuf> = context
                    .files
                    .iter()
                    .filter(|f| business(f))
                    .cloned()
                    .collect();
                files.extend(
                    code.slice(repo_root, &context.entry_files, 1, MAX_REFERENCED)
                        .into_iter()
                        .filter(|f| {
                            business(f) && owner.get(f.as_path()).is_none_or(|o| *o == index)
                        }),
                );
                files.sort();
                files.dedup();
                files
            })
            .collect();
        Self { read }
    }
}

/// The contexts of this run and the files to read for them. `named` are slugs (all of them are
/// taken, whatever the budget, done or not); otherwise the next contexts not done are taken while
/// their files fit `budget`, the first one always.
///
/// # Errors
///
/// Returns an error if a named context is not in the map.
pub fn plan_run(
    repo_root: &Path,
    map: &ContextMap,
    sources: &[PathBuf],
    seeds: &[PathBuf],
    roles: &RoleMap,
    named: &[String],
    budget: usize,
) -> Result<RunSlice, PipelineError> {
    let resolved = map.resolve(sources, seeds);
    for name in named {
        if !map.contexts.iter().any(|c| &c.slug == name) {
            return Err(PipelineError::UnknownContext {
                name: name.clone(),
                known: map
                    .contexts
                    .iter()
                    .map(|c| c.slug.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
    }
    let reading = Reading::of(repo_root, &resolved, roles, sources);
    let prints = Fingerprints::load(repo_root);
    let done: Vec<bool> = resolved
        .contexts
        .iter()
        .zip(&reading.read)
        .map(|(c, read)| prints.contexts.get(&c.slug) == Some(&context_hash(repo_root, c, read)))
        .collect();

    let mut selected: Vec<usize> = Vec::new();
    if named.is_empty() {
        let mut total = 0;
        for (i, context) in resolved
            .contexts
            .iter()
            .enumerate()
            .filter(|(i, _)| !done[*i])
        {
            if !selected.is_empty() && total + context.files.len() > budget {
                break;
            }
            total += context.files.len();
            selected.push(i);
        }
    } else {
        selected = (0..resolved.contexts.len())
            .filter(|i| named.contains(&resolved.contexts[*i].slug))
            .collect();
    }
    let chosen: BTreeSet<usize> = selected.iter().copied().collect();

    let mut files: Vec<PathBuf> = selected
        .iter()
        .flat_map(|&i| reading.read[i].iter().cloned())
        .collect();
    files.sort();
    files.dedup();
    let slugs = |keep: &dyn Fn(usize) -> bool| -> Vec<String> {
        (0..resolved.contexts.len())
            .filter(|i| keep(*i))
            .map(|i| resolved.contexts[i].slug.clone())
            .collect()
    };
    Ok(RunSlice {
        contexts: slugs(&|i| chosen.contains(&i)),
        files,
        pending: slugs(&|i| !done[i] && !chosen.contains(&i)),
        done: slugs(&|i| done[i]),
    })
}

/// Records that the whole chain of `slug` finished on the files as they are now, and forgets the
/// contexts the map no longer has.
///
/// # Errors
///
/// Returns an error if `slug` is not in the map or `fingerprints.json` can't be written.
pub fn mark_done(
    repo_root: &Path,
    map: &ContextMap,
    sources: &[PathBuf],
    seeds: &[PathBuf],
    roles: &RoleMap,
    slug: &str,
) -> Result<(), PipelineError> {
    let resolved = map.resolve(sources, seeds);
    let known = || {
        resolved
            .contexts
            .iter()
            .map(|c| c.slug.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let index = resolved
        .contexts
        .iter()
        .position(|c| c.slug == slug)
        .ok_or_else(|| PipelineError::UnknownContext {
            name: slug.to_string(),
            known: known(),
        })?;
    let reading = Reading::of(repo_root, &resolved, roles, sources);
    let mut prints = Fingerprints::load(repo_root);
    prints
        .contexts
        .retain(|name, _| resolved.contexts.iter().any(|c| &c.slug == name));
    prints.contexts.insert(
        slug.to_string(),
        context_hash(repo_root, &resolved.contexts[index], &reading.read[index]),
    );
    prints.save(repo_root)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
