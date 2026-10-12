//! The mechanical groups of the map of the contexts: no LLM call. One group per entry point file name
//! (the words that repeat one of its directories left out: `contracts_controller` under `controllers`
//! gives `contract`), and the other source files join the group whose words start their own name or
//! name one of their directories, the longest name first. What joins none is an orphan.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Lowercase singular words of a file or directory name (`CompaniesContracts`, `companies_contracts`
/// → `company`, `contract`).
fn words(name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for c in name.chars() {
        if !c.is_alphanumeric() || (c.is_uppercase() && previous_lower) {
            out.push(std::mem::take(&mut current));
        }
        if c.is_alphanumeric() {
            current.extend(c.to_lowercase());
        }
        previous_lower = c.is_lowercase();
    }
    out.push(current);
    out.into_iter()
        .filter(|w| !w.is_empty())
        .map(|w| singular(&w))
        .collect()
}

fn singular(word: &str) -> String {
    if let Some(stem) = word.strip_suffix("ies") {
        if stem.len() > 1 {
            return format!("{stem}y");
        }
    }
    // `statuses` → `status`, `addresses` → `address`, `boxes` → `box`, `branches` → `branch`.
    for plural in ["tuses", "sses", "xes", "tches", "nches", "rches", "shes"] {
        if let Some(stem) = word.strip_suffix("es").filter(|_| word.ends_with(plural)) {
            return stem.to_string();
        }
    }
    // `status`, `address`, `analysis` are already singular.
    if ["ss", "us", "is"].iter().any(|end| word.ends_with(end)) {
        return word.to_string();
    }
    match word.strip_suffix('s') {
        Some(stem) if stem.len() >= 3 => stem.to_string(),
        _ => word.to_string(),
    }
}

/// File stems that name a role in a folder, not a business object: the object is the folder's.
const GENERIC_STEMS: &[&str] = &[
    "index", "mod", "main", "init", "url", "urls", "route", "view", "handler",
];

/// The words of each directory above `file`, the nearest last.
fn directories(file: &Path) -> Vec<Vec<String>> {
    file.parent()
        .map(|dir| {
            dir.components()
                .map(|c| words(&c.as_os_str().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default()
}

/// The words of the name of `file` that do not repeat one of its directories; all of them when none
/// is left. A generic stem (`index`, `mod`, `urls`…) is replaced by the words of the nearest
/// directory that is not itself generic, which names the object (`src/contracts/index.ts` gives
/// `contract`).
#[must_use]
pub fn name_words(file: &Path) -> Vec<String> {
    let stem = file
        .file_stem()
        .map(|s| words(&s.to_string_lossy()))
        .unwrap_or_default();
    if stem.len() == 1 && GENERIC_STEMS.contains(&stem[0].as_str()) {
        let is_generic = |d: &Vec<String>| d.len() == 1 && GENERIC_STEMS.contains(&d[0].as_str());
        if let Some(dir) = directories(file)
            .into_iter()
            .rev()
            .find(|d| !d.is_empty() && !is_generic(d))
        {
            return dir;
        }
    }
    let layers: BTreeSet<String> = directories(file).into_iter().flatten().collect();
    let kept: Vec<String> = stem
        .iter()
        .filter(|w| !layers.contains(*w))
        .cloned()
        .collect();
    if kept.is_empty() {
        stem
    } else {
        kept
    }
}

/// The groups `file` joins by its name or its directories: the longest names that match, sorted.
#[must_use]
pub fn contexts_of(file: &Path, groups: &BTreeSet<Vec<String>>) -> Vec<Vec<String>> {
    let name = name_words(file);
    let mut matches: Vec<Vec<String>> = (1..=name.len())
        .map(|n| name[..n].to_vec())
        .chain(directories(file))
        .filter(|candidate| groups.contains(candidate))
        .collect();
    let longest = matches.iter().map(Vec::len).max().unwrap_or(0);
    matches.retain(|m| m.len() == longest);
    matches.sort();
    matches.dedup();
    matches
}

/// The name of a group as written in the map: its words joined by `_`.
pub(crate) fn group_name(words: &[String]) -> String {
    words.join("_")
}

/// The words of a group name written by [`group_name`].
pub(crate) fn group_words(name: &str) -> Vec<String> {
    name.split('_').map(ToString::to_string).collect()
}

/// The single group `file` joins: the longest name that matches, the first in sorted order on a tie.
pub(crate) fn group_of(file: &Path, groups: &BTreeSet<Vec<String>>) -> Option<Vec<String>> {
    contexts_of(file, groups).into_iter().next()
}

/// The source files by group, and those that join none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Groups {
    /// Group name (see [`group_name`]) → its files in path order, entry point files included.
    pub groups: BTreeMap<String, BTreeSet<PathBuf>>,
    pub orphans: Vec<PathBuf>,
}

/// Groups the `sources` around the names of the `seeds` (the entry point files, which are part of
/// `sources`).
#[must_use]
pub fn mechanical_groups(sources: &[PathBuf], seeds: &[PathBuf]) -> Groups {
    let seed_set: BTreeSet<&PathBuf> = seeds.iter().collect();
    let names: BTreeSet<Vec<String>> = seeds
        .iter()
        .map(|seed| name_words(seed))
        .filter(|name| !name.is_empty())
        .collect();
    let mut out = Groups::default();
    for seed in seeds {
        let name = name_words(seed);
        if !name.is_empty() {
            out.groups
                .entry(group_name(&name))
                .or_default()
                .insert(seed.clone());
        }
    }
    for file in sources.iter().filter(|f| !seed_set.contains(f)) {
        match group_of(file, &names) {
            Some(name) => {
                out.groups
                    .entry(group_name(&name))
                    .or_default()
                    .insert(file.clone());
            }
            None => out.orphans.push(file.clone()),
        }
    }
    out.orphans.sort();
    out
}
