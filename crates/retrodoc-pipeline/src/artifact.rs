//! Reading and writing the artifacts saved under `.retrodoc/cache/`: one
//! place for the "create the folder, serialize, write" sequence and for the
//! "missing or unreadable is a first run, not an error" rule.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::PipelineError;

/// Where the artifacts live, relative to the repo root.
const CACHE_DIR: &str = ".retrodoc/cache";

/// Every file the passes save under `.retrodoc/cache/`: the one place that
/// knows their names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Artifact {
    RepoMap,
    Fingerprints,
    Roles,
    SignalSources,
    Product,
    Glossary,
    EntryPoints,
    Actors,
    Scope,
    Domains,
    Features,
    UseCases,
    Usage,
}

impl Artifact {
    pub const ALL: [Artifact; 13] = [
        Artifact::RepoMap,
        Artifact::Fingerprints,
        Artifact::Roles,
        Artifact::SignalSources,
        Artifact::Product,
        Artifact::Glossary,
        Artifact::EntryPoints,
        Artifact::Actors,
        Artifact::Scope,
        Artifact::Domains,
        Artifact::Features,
        Artifact::UseCases,
        Artifact::Usage,
    ];

    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            Artifact::RepoMap => "repo-map.json",
            Artifact::Fingerprints => "fingerprints.json",
            Artifact::Roles => "roles.yaml",
            Artifact::SignalSources => "signal-sources.yaml",
            Artifact::Product => "product.yaml",
            Artifact::Glossary => "glossary.yaml",
            Artifact::EntryPoints => "entry-points.yaml",
            Artifact::Actors => "actors.yaml",
            Artifact::Scope => "scope.yaml",
            Artifact::Domains => "domains.yaml",
            Artifact::Features => "features.yaml",
            Artifact::UseCases => "use-cases.yaml",
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
    /// redo: `roles.yaml`, `signal-sources.yaml` and `product.yaml` are hand-editable (`retrodoc roles --force`
    /// identifies it again), `usage.json` is the history of the runs, and
    /// `domains.yaml` and `scope.yaml` are recomputed on every run anyway.
    #[must_use]
    pub fn cleared_by_force(self) -> bool {
        match self {
            Artifact::RepoMap
            | Artifact::Fingerprints
            | Artifact::Glossary
            | Artifact::EntryPoints
            | Artifact::Actors
            | Artifact::Features
            | Artifact::UseCases => true,
            Artifact::Roles
            | Artifact::SignalSources
            | Artifact::Product
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
    std::fs::write(path, raw).map_err(io_error)
}

#[cfg(test)]
mod tests {
    use super::*;

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
                "signal-sources.yaml",
                "product.yaml",
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
