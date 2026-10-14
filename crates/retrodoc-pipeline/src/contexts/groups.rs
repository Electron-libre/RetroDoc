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

/// A name is a family when it shows in at least this many layer folders...
const FAMILY_MIN_LAYERS: usize = 2;
/// ...and names at least this many files.
const FAMILY_MIN_FILES: usize = 3;
/// A name in more folders than this is a step every family has (`start`), not a family.
const FAMILY_MAX_LAYERS: usize = 8;

/// The groups of the orphans that a business family makes across layers: a name that repeats in
/// several folders (`rules/<family>.rb`, `processes/<family>/…`, `tasks/<family>_task.rb`), which the
/// entry point files can't give because the layer has none. A name needs [`FAMILY_MIN_LAYERS`]
/// folders and [`FAMILY_MIN_FILES`] files; the name of a layer folder, a generic stem and a name in
/// `taken` (a group already) are not families. A file joins the family of its longest matching name,
/// found again by [`group_of`], which is what lets the map keep the family as a plain group name.
#[must_use]
pub fn family_groups(
    orphans: &[PathBuf],
    taken: &BTreeSet<Vec<String>>,
) -> BTreeMap<String, BTreeSet<PathBuf>> {
    // Name → the folders it shows in.
    let mut layers: BTreeMap<Vec<String>, BTreeSet<PathBuf>> = BTreeMap::new();
    for file in orphans {
        let name = name_words(file);
        if !name.is_empty() {
            let parent = file.parent().map(Path::to_path_buf).unwrap_or_default();
            layers.entry(name).or_default().insert(parent);
        }
        let mut container = PathBuf::new();
        for component in file.parent().into_iter().flat_map(Path::components) {
            let dir = words(&component.as_os_str().to_string_lossy());
            if !dir.is_empty() {
                layers.entry(dir).or_default().insert(container.clone());
            }
            container.push(component);
        }
    }
    let is_generic =
        |name: &Vec<String>| name.len() == 1 && GENERIC_STEMS.contains(&name[0].as_str());
    layers.retain(|name, folders| {
        (FAMILY_MIN_LAYERS..=FAMILY_MAX_LAYERS).contains(&folders.len())
            && !is_generic(name)
            && !taken.contains(name)
    });
    // A folder that holds several of these names is a layer (`rules/` holds the families), and its
    // own name is not a family. A family also names files (`rules/<family>.rb`); a layer folder never
    // does.
    let mut held: BTreeMap<&PathBuf, usize> = BTreeMap::new();
    for folder in layers.values().flatten() {
        *held.entry(folder).or_insert(0) += 1;
    }
    let stems: BTreeSet<Vec<String>> = orphans.iter().map(|f| name_words(f)).collect();
    let layer_names: BTreeSet<Vec<String>> = held
        .into_iter()
        .filter(|(_, n)| *n >= FAMILY_MIN_LAYERS)
        .filter_map(|(folder, _)| folder.file_name())
        .map(|last| words(&last.to_string_lossy()))
        .filter(|name| !stems.contains(name))
        .collect();
    layers.retain(|name, _| !layer_names.contains(name));
    // A step every family has (`processes/<family>/send.rb`) shows in the folders of the families
    // themselves: when most of the folders of a name are named like another candidate, it is a step.
    let candidates: BTreeSet<Vec<String>> = layers.keys().cloned().collect();
    layers.retain(|_, folders| {
        let inside = folders
            .iter()
            .filter(|f| {
                f.file_name()
                    .is_some_and(|last| candidates.contains(&words(&last.to_string_lossy())))
            })
            .count();
        inside * 2 <= folders.len()
    });
    // A name that ends with too few files gives them back to the shorter names that match them.
    let mut names: BTreeSet<Vec<String>> = layers.into_keys().collect();
    loop {
        let mut out: BTreeMap<Vec<String>, BTreeSet<PathBuf>> = BTreeMap::new();
        for file in orphans {
            if let Some(name) = group_of(file, &names) {
                out.entry(name).or_default().insert(file.clone());
            }
        }
        let small: Vec<Vec<String>> = names
            .iter()
            .filter(|name| out.get(*name).map_or(0, BTreeSet::len) < FAMILY_MIN_FILES)
            .cloned()
            .collect();
        if small.is_empty() {
            return out
                .into_iter()
                .map(|(name, files)| (group_name(&name), files))
                .collect();
        }
        for name in small {
            names.remove(&name);
        }
    }
}

#[cfg(test)]
mod family_tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<PathBuf> {
        list.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_name_across_layers_is_a_family() {
        let orphans = paths(&[
            "app/workflows/rules/approval_flow.rb",
            "app/workflows/tasks/approval_flow_task.rb",
            "app/workflows/processes/approval_flow/start.rb",
            "app/workflows/processes/approval_flow/finish.rb",
            "app/other/rules/unrelated.rb",
            "app/workflows/rules/denial.rb",
            "app/workflows/tasks/denial_task.rb",
            "app/workflows/processes/denial/start.rb",
        ]);
        let families = family_groups(&orphans, &BTreeSet::new());
        let names: Vec<&str> = families.keys().map(String::as_str).collect();
        // `rules` shows in two folders but holds the families: a layer.
        assert_eq!(names, ["approval_flow", "denial"]);
        assert_eq!(families["approval_flow"].len(), 4);
        assert_eq!(families["denial"].len(), 3);
    }

    #[test]
    fn a_name_in_one_layer_a_layer_name_and_a_taken_group_are_not_families() {
        let orphans = paths(&[
            "app/workflows/rules/solo.rb",
            "app/workflows/rules/solo_two.rb",
            "app/workflows/rules/solo_three.rb",
            "app/workflows/tasks/a.rb",
            "app/workflows/processes/b.rb",
            "app/workflows/rules/known.rb",
            "app/workflows/tasks/known_task.rb",
            "app/workflows/processes/known/x.rb",
        ]);
        let taken = BTreeSet::from([vec!["known".to_string()]]);
        assert!(family_groups(&orphans, &taken).is_empty());
    }

    #[test]
    fn layers_repeated_under_two_roots_do_not_hide_a_family() {
        let mut orphans = Vec::new();
        for root in ["app/a", "app/b"] {
            for layer in ["rules", "tasks", "processes"] {
                for family in ["flow", "audit"] {
                    orphans.push(PathBuf::from(format!("{root}/{layer}/{family}.rb")));
                }
            }
        }
        let families = family_groups(&orphans, &BTreeSet::new());
        assert_eq!(families["flow"].len(), 6);
        assert_eq!(families["audit"].len(), 6);
    }

    #[test]
    fn a_step_every_family_has_is_not_a_family() {
        let orphans = paths(&[
            "w/rules/market.rb",
            "w/tasks/market_task.rb",
            "w/processes/market/send.rb",
            "w/processes/market/finish.rb",
            "w/rules/denial.rb",
            "w/tasks/denial_task.rb",
            "w/processes/denial/send.rb",
            "w/processes/denial/finish.rb",
        ]);
        let families = family_groups(&orphans, &BTreeSet::new());
        let names: Vec<&str> = families.keys().map(String::as_str).collect();
        assert_eq!(names, ["denial", "market"]);
        assert_eq!(families["market"].len(), 4);
    }

    #[test]
    fn a_family_under_three_files_is_dropped() {
        let orphans = paths(&["a/rules/pay.rb", "a/tasks/pay_task.rb"]);
        assert!(family_groups(&orphans, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn generic_stems_are_not_families() {
        let orphans = paths(&[
            "a/rules/index.rb",
            "a/tasks/index.rb",
            "a/processes/index.rb",
        ]);
        assert!(family_groups(&orphans, &BTreeSet::new()).is_empty());
    }
}
