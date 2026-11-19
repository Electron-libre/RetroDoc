//! The hash of each source file at the last complete `generate`, saved as
//! `.retrodoc/cache/source-hashes.json`: what the freshness of the docs compares the files against.
//! The pipeline captures them (`retrodoc_pipeline::source_hashes`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cache::{load_json, save_json, Artifact, CacheError};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceHashes {
    #[serde(default)]
    files: BTreeMap<String, String>,
}

impl SourceHashes {
    /// Adds the hashes of `other`, replacing those of the files both have: a run on some contexts
    /// of an application says nothing of the code of the others.
    pub fn merge(&mut self, other: Self) {
        self.files.extend(other.files);
    }

    /// Missing or unreadable (never generated, or `generate --force` running):
    /// no hash at all.
    #[must_use]
    pub fn load(repo_root: &Path) -> Self {
        load_json(&Artifact::SourceHashes.path(repo_root)).unwrap_or_default()
    }

    /// # Errors
    ///
    /// Returns an error if the file can't be written.
    pub fn save(&self, repo_root: &Path) -> Result<(), CacheError> {
        save_json(&Artifact::SourceHashes.path(repo_root), self)
    }

    #[must_use]
    pub fn get(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    /// The files hashed, in path order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// Sets the hash of `path`.
    pub fn put(&mut self, path: &str, hash: &str) {
        self.files.insert(path.to_string(), hash.to_string());
    }
}

/// The hashes of `(path, hash)` pairs, the last one of a path kept.
impl FromIterator<(String, String)> for SourceHashes {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(pairs: I) -> Self {
        Self {
            files: pairs.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_saved_means_no_hash() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(SourceHashes::load(dir.path()).get("a.rb"), None);
    }
}
