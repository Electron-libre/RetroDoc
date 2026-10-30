//! Reading and writing the artifacts saved under `.retrodoc/cache/`: one
//! place for the "create the folder, serialize, write" sequence and for the
//! "missing or unreadable is a first run, not an error" rule.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::PipelineError;

/// Where the artifacts live, relative to the repo root.
const CACHE_DIR: &str = ".retrodoc/cache";

/// What a reading pass (glossary, entry points) does with the saved entries of the files it is not
/// given this time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    /// Only the files read now stay: the inventory follows the files it is asked about.
    OnlyRead,
    /// The saved entries of the other files that still exist stay: a repository read in several
    /// runs, a slice at a time.
    Others,
}

/// Every file the passes save under `.retrodoc/cache/`: the one place that
/// knows their names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Artifact {
    SourceHashes,
    Fingerprints,
    Roles,
    /// The roles of the files no rule of `roles.yaml` matches, from their paths.
    RolesUnmatched,
    SignalSources,
    Product,
    BusinessFiles,
    Contexts,
    Glossary,
    EntryPoints,
    Actors,
    Scope,
    Domains,
    Features,
    UseCases,
    Rules,
    /// The rules as each feature stated them, before the same rule of several features is kept once;
    /// what a rerun starts from.
    RulesStated,
    /// What the rules pass left out on a judgement of the LLM (features about tests, rules
    /// dropped as noise), kept to check the doubtful cases.
    RulesSetAside,
    BehaviourCoverage,
    /// The verdicts of the reconciliation with the project's own docs.
    Reconciliation,
    Usage,
}

impl Artifact {
    pub const ALL: [Artifact; 21] = [
        Artifact::SourceHashes,
        Artifact::Fingerprints,
        Artifact::Roles,
        Artifact::RolesUnmatched,
        Artifact::SignalSources,
        Artifact::Product,
        Artifact::BusinessFiles,
        Artifact::Contexts,
        Artifact::Glossary,
        Artifact::EntryPoints,
        Artifact::Actors,
        Artifact::Scope,
        Artifact::Domains,
        Artifact::Features,
        Artifact::UseCases,
        Artifact::Rules,
        Artifact::RulesStated,
        Artifact::RulesSetAside,
        Artifact::BehaviourCoverage,
        Artifact::Reconciliation,
        Artifact::Usage,
    ];

    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            Artifact::SourceHashes => "source-hashes.json",
            Artifact::Fingerprints => "fingerprints.json",
            Artifact::Roles => "roles.yaml",
            Artifact::RolesUnmatched => "roles-unmatched.yaml",
            Artifact::SignalSources => "signal-sources.yaml",
            Artifact::Product => "product.yaml",
            Artifact::BusinessFiles => "business-files.yaml",
            Artifact::Contexts => "contexts.yaml",
            Artifact::Glossary => "glossary.yaml",
            Artifact::EntryPoints => "entry-points.yaml",
            Artifact::Actors => "actors.yaml",
            Artifact::Scope => "scope.yaml",
            Artifact::Domains => "domains.yaml",
            Artifact::Features => "features.yaml",
            Artifact::UseCases => "use-cases.yaml",
            Artifact::Rules => "rules.yaml",
            Artifact::RulesStated => "rules-stated.yaml",
            Artifact::RulesSetAside => "rules-set-aside.yaml",
            Artifact::BehaviourCoverage => "behaviour-coverage.yaml",
            Artifact::Reconciliation => "reconciliation.yaml",
            Artifact::Usage => "usage.json",
        }
    }

    /// The path as shown to the user, from the repo root.
    #[must_use]
    pub fn relative_path(self) -> String {
        format!("{CACHE_DIR}/{}", self.file_name())
    }

    #[must_use]
    pub fn path(self, repo_root: &Path) -> PathBuf {
        repo_root.join(CACHE_DIR).join(self.file_name())
    }

    /// Whether `generate --force` removes it. The others are not results to
    /// redo: `roles.yaml`, `roles-unmatched.yaml`, `signal-sources.yaml`, `product.yaml`, `business-files.yaml` and `contexts.yaml` are hand-editable (`retrodoc roles --force`
    /// identifies it again), `usage.json` is the history of the runs, and
    /// `domains.yaml` and `scope.yaml` are recomputed on every run anyway.
    #[must_use]
    pub fn cleared_by_force(self) -> bool {
        match self {
            Artifact::SourceHashes
            | Artifact::Fingerprints
            | Artifact::Glossary
            | Artifact::EntryPoints
            | Artifact::Actors
            | Artifact::Features
            | Artifact::UseCases
            | Artifact::Rules
            | Artifact::RulesStated
            | Artifact::RulesSetAside
            | Artifact::BehaviourCoverage
            | Artifact::Reconciliation => true,
            Artifact::Roles
            | Artifact::RolesUnmatched
            | Artifact::SignalSources
            | Artifact::Product
            | Artifact::BusinessFiles
            | Artifact::Contexts
            | Artifact::Scope
            | Artifact::Domains
            | Artifact::Usage => false,
        }
    }
}

/// Missing or unreadable (or no longer matching the type): `None`.
pub(crate) fn load_yaml<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_yaml::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Missing or unreadable (or no longer matching the type): `None`.
pub(crate) fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub(crate) fn save_yaml<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), PipelineError> {
    write(path, &serde_yaml::to_string(value)?)
}

pub(crate) fn save_json<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), PipelineError> {
    write(path, &serde_json::to_string_pretty(value)?)
}

/// For saves made while another error is already being propagated (partial
/// results of an interrupted pass): a failed write must not mask that error,
/// but it must not vanish either.
pub(crate) fn warn_on_error(result: Result<(), PipelineError>) {
    if let Err(error) = result {
        tracing::warn!("could not save an intermediate artifact: {error}");
    }
}

fn write(path: &Path, raw: &str) -> Result<(), PipelineError> {
    let io_error = |source| PipelineError::ArtifactIo {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_error)?;
    }
    // Written beside, synced, then renamed: a run killed during the write leaves the previous file
    // whole, never half of the new one, and a crash of the machine does not leave the new name on
    // data that never reached the disk.
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    let beside = path.with_file_name(name);
    let written = std::fs::File::create(&beside).and_then(|mut file| {
        std::io::Write::write_all(&mut file, raw.as_bytes())?;
        file.sync_all()
    });
    let moved = written.and_then(|()| std::fs::rename(&beside, path));
    if moved.is_err() {
        let _ = std::fs::remove_file(&beside);
    }
    moved.map_err(io_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: RER-6
    #[test]
    fn a_save_replaces_the_file_and_leaves_nothing_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache/thing.yaml");
        save_yaml(&path, &vec!["first"]).unwrap();
        save_yaml(&path, &vec!["second"]).unwrap();

        assert_eq!(load_yaml::<Vec<String>>(&path).unwrap(), ["second"]);
        let names: Vec<_> = std::fs::read_dir(dir.path().join("cache"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["thing.yaml"]);
    }

    #[test]
    fn artifacts_have_distinct_names_under_the_cache_dir() {
        let names: std::collections::BTreeSet<_> =
            Artifact::ALL.iter().map(|a| a.file_name()).collect();
        assert_eq!(names.len(), Artifact::ALL.len());
        let root = Path::new("/repo");
        assert_eq!(
            Artifact::Glossary.path(root),
            Path::new("/repo/.retrodoc/cache/glossary.yaml")
        );
        assert_eq!(
            Artifact::UseCases.relative_path(),
            ".retrodoc/cache/use-cases.yaml"
        );
    }

    // spec: RER-7
    #[test]
    fn force_clears_the_results_of_the_llm_passes_only() {
        let kept: Vec<_> = Artifact::ALL
            .iter()
            .filter(|a| !a.cleared_by_force())
            .map(|a| a.file_name())
            .collect();
        assert_eq!(
            kept,
            [
                "roles.yaml",
                "roles-unmatched.yaml",
                "signal-sources.yaml",
                "product.yaml",
                "business-files.yaml",
                "contexts.yaml",
                "scope.yaml",
                "domains.yaml",
                "usage.json"
            ]
        );
    }

    #[test]
    fn saves_create_the_folder_and_loads_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("a/b/x.yaml");
        let json = dir.path().join("c/x.json");
        save_yaml(&yaml, &vec!["one".to_string()]).unwrap();
        save_json(&json, &vec![1, 2]).unwrap();
        assert_eq!(load_yaml::<Vec<String>>(&yaml), Some(vec!["one".into()]));
        assert_eq!(load_json::<Vec<i32>>(&json), Some(vec![1, 2]));
    }

    #[test]
    fn missing_or_mismatching_files_load_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.yaml");
        assert_eq!(load_yaml::<Vec<String>>(&path), None);
        std::fs::write(&path, "not: a list").unwrap();
        assert_eq!(load_yaml::<Vec<String>>(&path), None);
    }
}
