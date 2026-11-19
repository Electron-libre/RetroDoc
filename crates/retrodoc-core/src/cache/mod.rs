//! The layout of `.retrodoc/cache/`, written by the pipeline and read by the readers of the generated
//! docs (ADR 0051): the names of its files, one place for the "create the folder, serialize, write"
//! sequence and for the "missing or unreadable is a first run, not an error" rule, the content hash
//! of the fingerprints and source hashes, and the format version of the cache ([`format`]).

pub mod format;

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// An artifact that can't be written.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("could not serialize an artifact to JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("could not serialize an artifact to YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("artifact unreadable/unwritable at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// SHA-256 of `content`, in hexadecimal.
#[must_use]
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Where the artifacts live, relative to the repo root.
const CACHE_DIR: &str = ".retrodoc/cache";

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
    /// The features the features pass marked technical, out of the functional docs.
    FeaturesSetAside,
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
    /// The format version of the cache (ADR 0050).
    Format,
}

impl Artifact {
    pub const ALL: [Artifact; 23] = [
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
        Artifact::FeaturesSetAside,
        Artifact::UseCases,
        Artifact::Rules,
        Artifact::RulesStated,
        Artifact::RulesSetAside,
        Artifact::BehaviourCoverage,
        Artifact::Reconciliation,
        Artifact::Usage,
        Artifact::Format,
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
            Artifact::FeaturesSetAside => "features-set-aside.yaml",
            Artifact::UseCases => "use-cases.yaml",
            Artifact::Rules => "rules.yaml",
            Artifact::RulesStated => "rules-stated.yaml",
            Artifact::RulesSetAside => "rules-set-aside.yaml",
            Artifact::BehaviourCoverage => "behaviour-coverage.yaml",
            Artifact::Reconciliation => "reconciliation.yaml",
            Artifact::Usage => "usage.json",
            Artifact::Format => "format.json",
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
    /// identifies it again), `usage.json` is the history of the runs, `format.json` says the format of
    /// the cache the kept files are in, and
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
            | Artifact::FeaturesSetAside
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
            | Artifact::Usage
            | Artifact::Format => false,
        }
    }
}

/// Missing or unreadable (or no longer matching the type): `None`.
#[must_use]
pub fn load_yaml<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_yaml::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Missing or unreadable (or no longer matching the type): `None`.
#[must_use]
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Writes `value` as YAML at `path`, creating its directory.
///
/// # Errors
///
/// The value can't be serialized or the file can't be written.
pub fn save_yaml<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), CacheError> {
    write(path, &serde_yaml::to_string(value)?)
}

/// Writes `value` as pretty JSON at `path`, creating its directory.
///
/// # Errors
///
/// The value can't be serialized or the file can't be written.
pub fn save_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), CacheError> {
    write(path, &serde_json::to_string_pretty(value)?)
}

fn write(path: &Path, raw: &str) -> Result<(), CacheError> {
    let io_error = |source| CacheError::Io {
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

    // spec: RER-6 (internal: the artifacts are written by LLM passes the binary cannot run without an LLM)
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
    fn hash_changes_with_content() {
        assert_ne!(hash_content("a"), hash_content("b"));
        assert_eq!(hash_content("a"), hash_content("a"));
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

    // spec: RER-7 (internal: the artifacts are written by LLM passes the binary cannot run without an LLM)
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
                "usage.json",
                "format.json"
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
