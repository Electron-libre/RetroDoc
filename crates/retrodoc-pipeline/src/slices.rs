//! Code slices (PLAN.md §7.1, phase 7 step 4b): the part of the code an entry
//! point traverses (controller → service → model), found by the identifiers
//! it references, so that a use case is derived from the code it actually
//! runs rather than from every file of its feature.
//!
//! This is a heuristic, not call tracing (hard in dynamic languages): an
//! identifier designates a file when it matches the file's name, ignoring
//! case, underscores and a plural `s` (`ContractSigner`, `contract_signer`
//! and `contract_signers` all designate `contract_signer.rb`). A name shared
//! by a few files designates the ones closest in the tree to the referencing
//! file (`app/models/contract.rb` rather than `lib/pdf/contract.rb` from
//! `app/controllers/contracts_controller.rb`); one shared by many designates
//! none (too ambiguous: `base`, `index`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::naming::normalize;
use crate::repo_map::read_file_lossy;

/// A name shared by more files than this designates none of them.
const MAX_FILES_PER_NAME: usize = 6;
/// Of the files sharing a name, how many it designates (the closest ones).
const DESIGNATED_PER_NAME: usize = 2;

/// Source files of the repo by normalized file name.
#[derive(Debug, Clone, Default)]
pub struct CodeIndex {
    by_name: BTreeMap<String, Vec<PathBuf>>,
}

/// Number of leading directories two paths have in common.
fn shared_prefix(a: &Path, b: &Path) -> usize {
    let dirs = |p: &Path| -> Vec<String> {
        p.parent()
            .map(|d| {
                d.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    dirs(a)
        .iter()
        .zip(dirs(b).iter())
        .take_while(|(x, y)| x == y)
        .count()
}

/// Identifiers of `content` with their number of occurrences.
fn identifiers(content: &str) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for word in content.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if word.len() >= 4 {
            *counts.entry(word).or_insert(0) += 1;
        }
    }
    counts
}

impl CodeIndex {
    pub fn new<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Self {
        let mut by_name: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for path in paths {
            if let Some(stem) = path.file_stem() {
                by_name
                    .entry(normalize(&stem.to_string_lossy()))
                    .or_default()
                    .push(path.to_path_buf());
            }
        }
        Self { by_name }
    }

    /// Files designated by the identifiers of `content` (the content of
    /// `from`), most mentioned first (then in path order), `exclude`d files
    /// left out.
    fn referenced(&self, content: &str, from: &Path, exclude: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
        let mut scores: BTreeMap<&PathBuf, usize> = BTreeMap::new();
        for (word, count) in identifiers(content) {
            let Some(files) = self.by_name.get(&normalize(word)) else {
                continue;
            };
            if files.len() > MAX_FILES_PER_NAME {
                continue;
            }
            let mut candidates: Vec<&PathBuf> =
                files.iter().filter(|f| !exclude.contains(*f)).collect();
            candidates.sort_by(|a, b| {
                shared_prefix(from, b)
                    .cmp(&shared_prefix(from, a))
                    .then_with(|| a.components().count().cmp(&b.components().count()))
                    .then_with(|| a.cmp(b))
            });
            for file in candidates.into_iter().take(DESIGNATED_PER_NAME) {
                *scores.entry(file).or_insert(0) += count;
            }
        }
        let mut ranked: Vec<(&PathBuf, usize)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        ranked.into_iter().map(|(file, _)| file.clone()).collect()
    }

    /// The files reachable from `starts` by following referenced
    /// identifiers up to `depth` hops, at most `max_files` of them (the
    /// starts are not included). Nearer files come first, and within a hop
    /// the most referenced ones.
    #[must_use]
    pub fn slice(
        &self,
        repo_root: &Path,
        starts: &[PathBuf],
        depth: usize,
        max_files: usize,
    ) -> Vec<PathBuf> {
        let mut seen: BTreeSet<PathBuf> = starts.iter().cloned().collect();
        let mut frontier: Vec<PathBuf> = starts.to_vec();
        let mut slice: Vec<PathBuf> = Vec::new();

        for _ in 0..depth {
            let mut hop: BTreeMap<PathBuf, usize> = BTreeMap::new();
            for file in &frontier {
                let Ok(content) = read_file_lossy(repo_root, file) else {
                    continue;
                };
                for (rank, found) in self
                    .referenced(&content, file, &seen)
                    .into_iter()
                    .enumerate()
                {
                    // Best (lowest) rank across the files of the frontier.
                    let entry = hop.entry(found).or_insert(usize::MAX);
                    *entry = (*entry).min(rank);
                }
            }
            let mut next: Vec<(PathBuf, usize)> = hop.into_iter().collect();
            next.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            frontier = Vec::new();
            for (file, _) in next {
                if slice.len() >= max_files {
                    return slice;
                }
                seen.insert(file.clone());
                slice.push(file.clone());
                frontier.push(file);
            }
        }
        slice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, content: &str) {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }

    fn index(paths: &[&str]) -> CodeIndex {
        CodeIndex::new(paths.iter().map(Path::new))
    }

    #[test]
    fn names_match_across_naming_conventions_and_plurals() {
        assert_eq!(
            normalize("ContractSigner"),
            normalize("contract_signer.rb".trim_end_matches(".rb"))
        );
        assert_eq!(normalize("contract_signers"), normalize("ContractSigner"));
        assert_eq!(normalize("bus"), "bus");
    }

    #[test]
    fn follows_references_hop_by_hop_and_ignores_ambiguous_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "app/controllers/contracts_controller.rb",
            "def sign\n  ContractSigner.new(contract).call\n  Base.log\nend",
        );
        write(
            root,
            "app/services/contract_signer.rb",
            "class ContractSigner\n  def call\n    Signature.create!\n  end\nend",
        );
        write(root, "app/models/signature.rb", "class Signature; end");
        write(root, "app/models/contract.rb", "class Contract; end");
        for dir in ["a", "b", "c", "d", "e", "f", "g"] {
            write(root, &format!("app/{dir}/base.rb"), "x");
        }
        let paths = [
            "app/controllers/contracts_controller.rb",
            "app/services/contract_signer.rb",
            "app/models/signature.rb",
            "app/models/contract.rb",
        ];
        let mut paths = paths.to_vec();
        let bases: Vec<String> = ["a", "b", "c", "d", "e", "f", "g"]
            .iter()
            .map(|d| format!("app/{d}/base.rb"))
            .collect();
        paths.extend(bases.iter().map(String::as_str));
        let index = index(&paths);
        let starts = [PathBuf::from("app/controllers/contracts_controller.rb")];

        let one_hop = index.slice(root, &starts, 1, 10);
        assert_eq!(
            one_hop,
            vec![
                PathBuf::from("app/models/contract.rb"),
                PathBuf::from("app/services/contract_signer.rb"),
            ]
        );

        let two_hops = index.slice(root, &starts, 2, 10);
        assert!(two_hops.contains(&PathBuf::from("app/models/signature.rb")));
        assert!(!two_hops.iter().any(|p| p.ends_with("base.rb")));

        assert_eq!(index.slice(root, &starts, 2, 1).len(), 1);
    }

    #[test]
    fn a_shared_name_designates_the_closest_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "app/controllers/contracts_controller.rb",
            "Contract.find",
        );
        for path in [
            "app/models/contract.rb",
            "lib/pdf/contract.rb",
            "lib/xml/contract.rb",
        ] {
            write(root, path, "x");
        }
        let index = index(&[
            "app/controllers/contracts_controller.rb",
            "app/models/contract.rb",
            "lib/pdf/contract.rb",
            "lib/xml/contract.rb",
        ]);
        let starts = [PathBuf::from("app/controllers/contracts_controller.rb")];
        let slice = index.slice(root, &starts, 1, 1);
        assert_eq!(slice, vec![PathBuf::from("app/models/contract.rb")]);
    }
}
