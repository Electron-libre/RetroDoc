//! Incremental repo map cache (PLAN.md §2, risks §6): avoids re-querying
//! the LLM for a file whose content hasn't changed since the last run.
//! Key = SHA-256 hash of the file content, as planned in PLAN.md §4
//! (`.retrodoc/cache/`).
//!
//! Full formalization of the incremental cache (transitive invalidation via
//! `domains.yaml`) is left to the "domains" phase (PLAN.md §2 step 3): here,
//! only the file level is cached — module summaries are cheap (one per
//! folder) and recomputed on every run.

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    content_hash: String,
    role_summary: String,
}

/// Content hash used as the cache key (PLAN.md §4).
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl RepoMapCache {
    /// Loads the cache from `<repo_root>/.retrodoc/cache/repo-map.json`.
    /// Missing or unreadable: empty cache, not an error (first run).
    pub fn load(repo_root: &Path) -> Self {
        let path = repo_root.join(CACHE_RELATIVE_PATH);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

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
    pub fn get(&self, path: &Path, content_hash: &str) -> Option<&str> {
        self.entries
            .get(path)
            .filter(|entry| entry.content_hash == content_hash)
            .map(|entry| entry.role_summary.as_str())
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
