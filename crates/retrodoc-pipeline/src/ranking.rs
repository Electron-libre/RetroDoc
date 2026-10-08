//! Budget on the number of source files analysed (PLAN.md §7.2, phase 8):
//! on a large repo, spend the LLM calls on the files that matter most.
//!
//! Files are ranked mechanically (no LLM call) by `role weight × (1 + ln(1 +
//! commits) + ln(1 + references))`: the role comes from the role rules
//! (entry points and business logic first, config and assets last), the
//! commits from the git history (what changes is what lives), and the
//! references count the other files that mention the file's name (what
//! others depend on). The files left out are turned into [`FileKind::Other`]
//! so no later pass sees them, and listed in `.retrodoc/cache/scope.yaml`
//! for the debt report.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use retrodoc_ingest::{FileKind, IngestResult};
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::error::PipelineError;
use crate::naming::normalize;
use crate::repo_map::read_file_lossy;
use crate::roles::{FileRole, RoleMap};

/// A file name shared by more files than this designates none of them.
const MAX_SHARED_NAME: usize = 3;
/// Shortest identifier counted as a possible reference to a file.
const MIN_TOKEN_LEN: usize = 4;
/// Only the start of a file is scanned for references.
const MAX_SCAN_CHARS: usize = 100_000;

/// What the budget kept and left out, as saved in `scope.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    /// The budget of this run.
    pub max_files: usize,
    /// Source files analysed.
    pub analysed: usize,
    /// Source files left out, in path order.
    pub skipped: Vec<PathBuf>,
}

impl Scope {
    /// Missing or unreadable: `None`.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Scope.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the file can't be written.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::Scope.path(repo_root), self)
    }

    /// Removes the saved scope (no budget in this run).
    pub fn clear(repo_root: &Path) {
        let _ = std::fs::remove_file(Artifact::Scope.path(repo_root));
    }
}

fn role_weight(role: Option<FileRole>) -> f32 {
    match role {
        Some(FileRole::EntryPoint | FileRole::Logic) => 3.0,
        Some(FileRole::Model) => 2.5,
        Some(FileRole::View | FileRole::Unclassified) | None => 1.0,
        Some(FileRole::Infra) => 0.7,
        Some(FileRole::Config | FileRole::Other | FileRole::Docs | FileRole::Test) => 0.3,
    }
}

/// File name as references write it: lowercase, no `_`/`-`, no plural `s`.
fn normalized_stem(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    normalize(stem)
}

/// Normalized identifiers of a text.
fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| t.len() >= MIN_TOKEN_LEN)
        .map(normalize)
        .collect()
}

/// Source files with their score, best first (ties in path order).
#[must_use]
pub fn rank_files(
    repo_root: &Path,
    ingest: &IngestResult,
    roles: Option<&RoleMap>,
) -> Vec<(PathBuf, f32)> {
    let sources: Vec<&Path> = ingest
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path.as_path())
        .collect();

    let mut owners: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, path) in sources.iter().enumerate() {
        owners.entry(normalized_stem(path)).or_default().push(i);
    }
    // For each file, how many *other* files mention its name.
    let mut references = vec![0_u32; sources.len()];
    for (i, path) in sources.iter().enumerate() {
        let Ok(content) = read_file_lossy(repo_root, path) else {
            continue;
        };
        let head: String = content.chars().take(MAX_SCAN_CHARS).collect();
        for token in tokens(&head) {
            if let Some(files) = owners.get(&token).filter(|f| f.len() <= MAX_SHARED_NAME) {
                for &owner in files.iter().filter(|&&o| o != i) {
                    references[owner] += 1;
                }
            }
        }
    }

    let mut ranked: Vec<(PathBuf, f32)> = sources
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let commits = ingest.history_for(path).map_or(0, |h| h.commit_count);
            let role = roles.and_then(|r| r.roles.get(*path).copied());
            let score = role_weight(role)
                * (1.0
                    + f32::from(u16::try_from(commits).unwrap_or(u16::MAX)).ln_1p()
                    + f32::from(u16::try_from(references[i]).unwrap_or(u16::MAX)).ln_1p());
            ((*path).to_path_buf(), score)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

/// Keeps the `max_files` best-ranked source files; the others become
/// [`FileKind::Other`] in `ingest`. Returns what was kept and left out.
pub fn apply_budget(
    repo_root: &Path,
    ingest: &mut IngestResult,
    roles: Option<&RoleMap>,
    max_files: usize,
) -> Scope {
    let ranked = rank_files(repo_root, ingest, roles);
    let kept: BTreeSet<&PathBuf> = ranked.iter().take(max_files).map(|(p, _)| p).collect();
    let mut skipped: Vec<PathBuf> = ranked
        .iter()
        .skip(max_files)
        .map(|(p, _)| p.clone())
        .collect();
    skipped.sort();
    let analysed = kept.len();
    let skipped_set: BTreeSet<&PathBuf> = skipped.iter().collect();
    for file in &mut ingest.files {
        if file.kind == FileKind::Source && skipped_set.contains(&file.path) {
            file.kind = FileKind::Other;
        }
    }
    Scope {
        max_files,
        analysed,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use retrodoc_ingest::{FileEntry, FileHistory};

    fn ingest(root: &Path, files: &[(&str, &str)]) -> IngestResult {
        let mut entries = Vec::new();
        for (path, content) in files {
            let abs = root.join(path);
            std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
            std::fs::write(&abs, content).unwrap();
            entries.push(FileEntry {
                path: PathBuf::from(path),
                kind: FileKind::Source,
                size_bytes: content.len() as u64,
            });
        }
        IngestResult {
            files: entries,
            history_by_path: HashMap::new(),
            existing_docs: Vec::new(),
            commits: Vec::new(),
        }
    }

    fn history(commits: u32) -> FileHistory {
        FileHistory {
            commit_count: commits,
            ..FileHistory::default()
        }
    }

    #[test]
    fn referenced_and_often_changed_files_rank_first() {
        let dir = tempfile::tempdir().unwrap();
        let mut ing = ingest(
            dir.path(),
            &[
                ("a/lonely.rb", "x = 1"),
                ("a/hub.rb", "class Hub; end"),
                ("a/user_a.rb", "Hub.call"),
                ("a/user_b.rb", "Hub.call; Churny.run"),
                ("a/churny.rb", "class Churny; end"),
            ],
        );
        ing.history_by_path
            .insert(PathBuf::from("a/churny.rb"), history(40));

        let ranked: Vec<String> = rank_files(dir.path(), &ing, None)
            .into_iter()
            .map(|(p, _)| p.display().to_string())
            .collect();

        // churny: 40 commits + 1 reference; hub: 0 commits + 2 references.
        assert_eq!(ranked[0], "a/churny.rb");
        assert_eq!(ranked[1], "a/hub.rb");
        assert_eq!(ranked.last().unwrap(), "a/user_b.rb");
    }

    #[test]
    fn role_weights_logic_above_config() {
        let dir = tempfile::tempdir().unwrap();
        let ing = ingest(dir.path(), &[("a.rb", "x"), ("b.rb", "x")]);
        let roles = RoleMap {
            roles: BTreeMap::from([
                (PathBuf::from("a.rb"), FileRole::Config),
                (PathBuf::from("b.rb"), FileRole::Logic),
            ]),
            ..RoleMap::default()
        };
        let ranked = rank_files(dir.path(), &ing, Some(&roles));
        assert_eq!(ranked[0].0, PathBuf::from("b.rb"));
    }

    #[test]
    fn budget_demotes_the_rest_to_other_and_is_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut ing = ingest(dir.path(), &[("a.rb", "x"), ("b.rb", "x"), ("c.rb", "x")]);
        ing.history_by_path
            .insert(PathBuf::from("b.rb"), history(9));

        let scope = apply_budget(dir.path(), &mut ing, None, 1);

        assert_eq!(scope.analysed, 1);
        assert_eq!(
            scope.skipped,
            vec![PathBuf::from("a.rb"), PathBuf::from("c.rb")]
        );
        let kinds: Vec<_> = ing.files.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            vec![FileKind::Other, FileKind::Source, FileKind::Other]
        );
        scope.save(dir.path()).unwrap();
        assert_eq!(Scope::load(dir.path()), Some(scope));
        Scope::clear(dir.path());
        assert!(Scope::load(dir.path()).is_none());
    }

    #[test]
    fn names_shared_by_many_files_designate_none() {
        let dir = tempfile::tempdir().unwrap();
        let ing = ingest(
            dir.path(),
            &[
                ("a/index.rb", ""),
                ("b/index.rb", ""),
                ("c/index.rb", ""),
                ("d/index.rb", ""),
                ("e/user.rb", "index"),
            ],
        );
        let ranked = rank_files(dir.path(), &ing, None);
        // Nobody gets a reference score, so they all tie and keep path order.
        assert!(ranked
            .iter()
            .all(|(_, score)| (*score - ranked[0].1).abs() < f32::EPSILON));
        assert_eq!(ranked[0].0, PathBuf::from("a/index.rb"));
    }
}
