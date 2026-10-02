//! Incremental repo map cache (PLAN.md §2, risks §6): avoids re-querying
//! the LLM for a file whose content hasn't changed since the last run.
//! Key = SHA-256 hash of the file content, as planned in PLAN.md §4
//! (`.retrodoc/cache/`).
//!
//! Module (folder) summaries are cached too (phase 8): the key is the hash of
//! the exact listing sent to the LLM, i.e. of the children's summaries, so a
//! changed file invalidates its folder and every ancestor, and nothing else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::PipelineError;

const CACHE_RELATIVE_PATH: &str = ".retrodoc/cache/repo-map.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RepoMapCache {
    #[serde(default)]
    entries: BTreeMap<PathBuf, CacheEntry>,
    #[serde(default)]
    modules: BTreeMap<PathBuf, ModuleCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleCacheEntry {
    input_hash: String,
    role_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    content_hash: String,
    role_summary: String,
}

/// Content hash used as the cache key (PLAN.md §4).
#[must_use]
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl RepoMapCache {
    /// Loads the cache from `<repo_root>/.retrodoc/cache/repo-map.json`.
    /// Missing or unreadable: empty cache, not an error (first run).
    #[must_use]
    pub fn load(repo_root: &Path) -> Self {
        let path = repo_root.join(CACHE_RELATIVE_PATH);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// # Errors
    ///
    /// Returns an error if the `.retrodoc/cache/` folder can't be created,
    /// or if writing the cache file fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        let path = repo_root.join(CACHE_RELATIVE_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PipelineError::Cache {
                path: path.clone(),
                source,
            })?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, raw).map_err(|source| PipelineError::Cache { path, source })?;
        Ok(())
    }

    /// Cached summary for `path`, if it still matches `content_hash` (file
    /// unchanged since the last run).
    #[must_use]
    pub fn get(&self, path: &Path, content_hash: &str) -> Option<&str> {
        self.entries
            .get(path)
            .filter(|entry| entry.content_hash == content_hash)
            .map(|entry| entry.role_summary.as_str())
    }

    /// Cached summary for the folder `dir`, if `input_hash` (hash of what
    /// was sent to the LLM) is unchanged since the last run.
    #[must_use]
    pub fn get_module(&self, dir: &Path, input_hash: &str) -> Option<&str> {
        self.modules
            .get(dir)
            .filter(|entry| entry.input_hash == input_hash)
            .map(|entry| entry.role_summary.as_str())
    }

    pub fn put_module(&mut self, dir: &Path, input_hash: &str, role_summary: &str) {
        self.modules.insert(
            dir.to_path_buf(),
            ModuleCacheEntry {
                input_hash: input_hash.to_string(),
                role_summary: role_summary.to_string(),
            },
        );
    }

    pub fn put(&mut self, path: &Path, content_hash: &str, role_summary: &str) {
        self.entries.insert(
            path.to_path_buf(),
            CacheEntry {
                content_hash: content_hash.to_string(),
                role_summary: role_summary.to_string(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = RepoMapCache::load(dir.path());
        assert!(cache.get(Path::new("a.rs"), "hash1").is_none());

        cache.put(Path::new("a.rs"), "hash1", "summary of a.rs");
        cache.save(dir.path()).unwrap();

        let reloaded = RepoMapCache::load(dir.path());
        assert_eq!(
            reloaded.get(Path::new("a.rs"), "hash1"),
            Some("summary of a.rs")
        );
        // Changed content (different hash): no more hit.
        assert!(reloaded.get(Path::new("a.rs"), "hash2").is_none());
    }

    #[test]
    fn hash_changes_with_content() {
        assert_ne!(hash_content("a"), hash_content("b"));
        assert_eq!(hash_content("a"), hash_content("a"));
    }
}
