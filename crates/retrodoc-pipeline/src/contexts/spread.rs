//! The files of a layer context spread over the contexts that use them (`specs/context-map.md`,
//! CTX-5). A context the answer made mostly of orphan folders (UI components, a folder of models, a
//! library) is a layer, not a business area: each of its folder files goes to the context whose files
//! write its name the most, a view or a model no other context writes goes to the support, and the
//! others (the tasks of a workflow, written by their own context) stay. A layer folder mixes files
//! that serve different contexts, so the files move one by one, listed in the map; no LLM call.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::groups::GENERIC_STEMS;
use super::{BoundedContext, ContextMap, Resolved, Support};
use crate::roles::{FileRole, RoleMap};

/// The identifiers each source file writes, read once from the tree (a file that can't be read
/// writes none), and the files of a shared layer (views and models).
pub(super) struct Written {
    identifiers: BTreeMap<PathBuf, BTreeSet<String>>,
    shared: BTreeSet<PathBuf>,
}

impl Written {
    pub(super) fn read(repo_root: &Path, sources: &[PathBuf], roles: &RoleMap) -> Self {
        let identifiers = sources
            .iter()
            .map(|file| {
                let text = std::fs::read_to_string(repo_root.join(file)).unwrap_or_default();
                (file.clone(), identifiers(&text))
            })
            .collect();
        let shared = sources
            .iter()
            .filter(|f| matches!(roles.roles.get(*f), Some(FileRole::View | FileRole::Model)))
            .cloned()
            .collect();
        Self {
            identifiers,
            shared,
        }
    }

    fn writes(&self, file: &Path, name: &str) -> bool {
        self.identifiers
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

/// The name a file is written under as a class: `card_component.rb` gives `CardComponent`,
/// `CardList.tsx` stays `CardList`. A generic stem (`index`, `mod`…) names nothing.
fn class_name(file: &Path) -> Option<String> {
    let stem = file.file_stem()?.to_string_lossy();
    if GENERIC_STEMS.contains(&stem.to_lowercase().as_str()) {
        return None;
    }
    let name: String = stem
        .split(['_', '-'])
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The `map` with the folder files of each layer context (more than one file, more than half of
/// them from its folders) listed one by one: in the other context whose files write its name the
/// most (the first in the map on a tie), else in the support for a view or a model; the others
/// stay. A layer context left without any file is empty and dropped after.
pub(super) fn spread_layer_contexts(
    map: ContextMap,
    sources: &[PathBuf],
    seeds: &[PathBuf],
    written: &Written,
) -> ContextMap {
    let resolved = map.resolve(sources, seeds);
    let by_group = groups_only(&map).resolve(sources, seeds);
    let mut moves: Vec<(PathBuf, Option<usize>)> = Vec::new();
    for (i, (all, grouped)) in resolved.contexts.iter().zip(&by_group.contexts).enumerate() {
        let folder_files: Vec<&PathBuf> = all
            .files
            .iter()
            .filter(|f| !grouped.files.contains(f))
            .collect();
        if all.files.len() < 2 || folder_files.len() * 2 <= all.files.len() {
            continue;
        }
        for file in folder_files {
            match most_written(file, &resolved, written, |j| j != i) {
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

/// The context among those `eligible` whose files write the class name of `file` the most, the
/// first on a tie; `None` when none writes it.
fn most_written(
    file: &Path,
    resolved: &Resolved,
    written: &Written,
    eligible: impl Fn(usize) -> bool,
) -> Option<usize> {
    let name = class_name(file)?;
    let mut best: Option<(usize, usize)> = None;
    for (j, context) in resolved.contexts.iter().enumerate() {
        if !eligible(j) {
            continue;
        }
        let count = context
            .files
            .iter()
            .filter(|f| written.writes(f, &name))
            .count();
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
    }
}
