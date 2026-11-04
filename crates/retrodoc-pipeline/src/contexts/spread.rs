//! The files of a layer context spread over the contexts that use them (`specs/context-map.md`,
//! CTX-5). A context the answer made mostly of orphan folders (UI components, a folder of models, a
//! library) is a layer, not a business area: each of its folder files goes to the context whose files
//! write its name the most, a view or a model no other context writes goes to the support, and the
//! others (the tasks of a workflow, written by their own context) stay. A layer folder mixes files
//! that serve different contexts, so the files move one by one, listed in the map. A catch-all
//! context (`misc`, `other`…) is spread the same way, its groups moving whole (CTX-6). No LLM call.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::groups::GENERIC_STEMS;
use super::{add_group, capitalized, BoundedContext, ContextMap, Resolved, Support};
use crate::roles::{FileRole, RoleMap};

/// The identifiers each source file writes, read from the tree the first time one is asked (a map
/// with no layer nor catch-all context reads nothing; a file that can't be read writes none), and
/// the files of a shared layer (views and models).
pub(super) struct Written {
    repo_root: PathBuf,
    sources: Vec<PathBuf>,
    identifiers: std::cell::OnceCell<BTreeMap<PathBuf, BTreeSet<String>>>,
    shared: BTreeSet<PathBuf>,
}

impl Written {
    pub(super) fn read(repo_root: &Path, sources: &[PathBuf], roles: &RoleMap) -> Self {
        let shared = sources
            .iter()
            .filter(|f| matches!(roles.roles.get(*f), Some(FileRole::View | FileRole::Model)))
            .cloned()
            .collect();
        Self {
            repo_root: repo_root.to_path_buf(),
            sources: sources.to_vec(),
            identifiers: std::cell::OnceCell::new(),
            shared,
        }
    }

    fn writes(&self, file: &Path, name: &str) -> bool {
        self.identifiers
            .get_or_init(|| {
                self.sources
                    .iter()
                    .map(|file| {
                        let text =
                            std::fs::read_to_string(self.repo_root.join(file)).unwrap_or_default();
                        (file.clone(), identifiers(&text))
                    })
                    .collect()
            })
            .get(file)
            .is_some_and(|names| names.contains(name))
    }
}

fn identifiers(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with(|c: char| c.is_alphabetic()))
        .map(ToString::to_string)
        .collect()
}

/// The name a file is written under as a class: `card_component.rb` and its template
/// `card_component.html.erb` give `CardComponent`, `CardList.tsx` stays `CardList`. A generic stem
/// (`index`, `mod`…) names nothing.
fn class_name(file: &Path) -> Option<String> {
    let file_name = file.file_name()?.to_string_lossy();
    let stem = file_name.split('.').next()?;
    if GENERIC_STEMS.contains(&stem.to_lowercase().as_str()) {
        return None;
    }
    let name: String = stem.split(['_', '-']).map(capitalized).collect();
    (!name.is_empty()).then_some(name)
}

/// The `map` with the folder files of each layer context (more than one file, more than half of
/// them from its folders) listed one by one: in the other context whose files write its name the
/// most (the first in the map on a tie), else in the support for a view or a model; the others
/// stay; a file already listed is neither counted as a folder file nor moved again. A layer context
/// left without any file is empty and dropped after.
pub(super) fn spread_layer_contexts(
    map: ContextMap,
    sources: &[PathBuf],
    seeds: &[PathBuf],
    written: &Written,
) -> ContextMap {
    let resolved = map.resolve(sources, seeds);
    let by_group = groups_only(&map).resolve(sources, seeds);
    let listed: BTreeSet<PathBuf> = map
        .contexts
        .iter()
        .flat_map(|c| &c.files)
        .chain(&map.support.files)
        .map(PathBuf::from)
        .collect();
    let mut moves: Vec<(PathBuf, Option<usize>)> = Vec::new();
    for (i, (all, grouped)) in resolved.contexts.iter().zip(&by_group.contexts).enumerate() {
        let grouped: BTreeSet<&PathBuf> = grouped.files.iter().collect();
        let folder_files: Vec<&PathBuf> = all
            .files
            .iter()
            .filter(|f| !grouped.contains(f) && !listed.contains(*f))
            .collect();
        if all.files.len() < 2 || folder_files.len() * 2 <= all.files.len() {
            continue;
        }
        for file in folder_files {
            match most_written(&[file], &resolved, written, |j| j != i) {
                Some(j) => moves.push((file.clone(), Some(j))),
                None if written.shared.contains(file) => moves.push((file.clone(), None)),
                None => {}
            }
        }
    }
    let ContextMap {
        mut contexts,
        mut support,
    } = map;
    for (file, to) in moves {
        let file = file.to_string_lossy().into_owned();
        tracing::debug!(%file, into = to.map_or("support", |j| contexts[j].slug.as_str()), "file of a layer context moved");
        match to {
            Some(j) => contexts[j].files.push(file),
            None => support.files.push(file),
        }
    }
    ContextMap { contexts, support }
}

/// Words a catch-all context is named with, with `business` or not.
const CATCH_ALL_WORDS: &[&str] = &[
    "misc",
    "miscellaneous",
    "other",
    "others",
    "general",
    "common",
    "shared",
    "various",
];

fn is_catch_all(slug: &str) -> bool {
    let words: Vec<&str> = slug.split('-').filter(|w| *w != "business").collect();
    !words.is_empty() && words.iter().all(|w| CATCH_ALL_WORDS.contains(w))
}

/// The `map` without its catch-all contexts (a slug of catch-all words only): each group goes to
/// the other context whose files write the class names of its files the most, else to a context of
/// its own; each folder file is listed in the context that writes its name the most, else in the
/// support. A catch-all context is left empty and dropped after.
pub(super) fn spread_catch_alls(
    map: ContextMap,
    sources: &[PathBuf],
    seeds: &[PathBuf],
    written: &Written,
) -> ContextMap {
    let catch_all: Vec<bool> = map.contexts.iter().map(|c| is_catch_all(&c.slug)).collect();
    if !catch_all.contains(&true) {
        return map;
    }
    let resolved = map.resolve(sources, seeds);
    let target = |j: usize| !catch_all[j];
    // Each group as a context of its own: the files `resolve` gives it.
    let one_per_group = ContextMap {
        contexts: map
            .contexts
            .iter()
            .flat_map(|c| &c.groups)
            .map(|g| BoundedContext {
                groups: vec![g.clone()],
                ..BoundedContext::default()
            })
            .collect(),
        // The support groups keep their names: a file of theirs never falls back to a group of a
        // context.
        support: Support {
            groups: map.support.groups.clone(),
            ..Support::default()
        },
    }
    .resolve(sources, seeds);
    let files_of_group = |k: usize| one_per_group.contexts[k].files.iter().collect::<Vec<_>>();
    let mut group_moves: Vec<(String, Option<usize>)> = Vec::new();
    let mut file_moves: Vec<(PathBuf, Option<usize>)> = Vec::new();
    let mut k = 0;
    for (i, context) in map.contexts.iter().enumerate() {
        for group in &context.groups {
            if catch_all[i] {
                let to = most_written(&files_of_group(k), &resolved, written, target);
                group_moves.push((group.clone(), to));
            }
            k += 1;
        }
    }
    let grouped: BTreeSet<&PathBuf> = one_per_group
        .contexts
        .iter()
        .flat_map(|c| &c.files)
        .collect();
    for (own, _) in resolved
        .contexts
        .iter()
        .zip(&catch_all)
        .filter(|(_, c)| **c)
    {
        for file in own.files.iter().filter(|f| !grouped.contains(f)) {
            file_moves.push((
                file.clone(),
                most_written(&[file], &resolved, written, target),
            ));
        }
    }
    let ContextMap {
        mut contexts,
        mut support,
    } = map;
    for (context, _) in contexts.iter_mut().zip(&catch_all).filter(|(_, c)| **c) {
        tracing::info!(context = %context.slug, "catch-all context spread");
        context.groups.clear();
        context.folders.clear();
        context.files.clear();
    }
    for (file, to) in file_moves {
        let file = file.to_string_lossy().into_owned();
        match to {
            Some(j) => contexts[j].files.push(file),
            None => support.files.push(file),
        }
    }
    for (group, to) in group_moves {
        match to {
            Some(j) => contexts[j].groups.push(group),
            None => add_group(&mut contexts, &group),
        }
    }
    ContextMap { contexts, support }
}

/// The map with its groups only: what [`ContextMap::resolve`] places by group, not by folder.
fn groups_only(map: &ContextMap) -> ContextMap {
    ContextMap {
        contexts: map
            .contexts
            .iter()
            .map(|c| BoundedContext {
                folders: Vec::new(),
                files: Vec::new(),
                ..c.clone()
            })
            .collect(),
        support: Support {
            groups: map.support.groups.clone(),
            ..Support::default()
        },
    }
}

/// The context among those `eligible` whose files write the class names of `files` the most, the
/// first on a tie; `None` when none writes one.
pub(super) fn most_written(
    files: &[&PathBuf],
    resolved: &Resolved,
    written: &Written,
    eligible: impl Fn(usize) -> bool,
) -> Option<usize> {
    let names: Vec<String> = files.iter().filter_map(|f| class_name(f)).collect();
    let mut best: Option<(usize, usize)> = None;
    for (j, context) in resolved.contexts.iter().enumerate() {
        if !eligible(j) {
            continue;
        }
        let count = context
            .files
            .iter()
            .map(|f| names.iter().filter(|n| written.writes(f, n)).count())
            .sum::<usize>();
        if count > 0 && best.is_none_or(|(_, most)| count > most) {
            best = Some((j, count));
        }
    }
    best.map(|(j, _)| j)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_written_under_its_class_name() {
        assert_eq!(
            class_name(Path::new("app/components/card_component.rb")).as_deref(),
            Some("CardComponent")
        );
        assert_eq!(
            class_name(Path::new("src/CardList.tsx")).as_deref(),
            Some("CardList")
        );
        assert_eq!(class_name(Path::new("src/users/index.ts")), None);
        assert_eq!(
            class_name(Path::new("app/components/card_component.html.erb")).as_deref(),
            Some("CardComponent")
        );
    }
}
