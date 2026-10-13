//! The hash of each source file as the docs were produced from it, saved in
//! `source-hashes.json` once a `generate` has gone through every pass.
//!
//! The MCP freshness check compares these with the files on disk. The repo map
//! cache can't serve for that: it is updated early in a run, so a run that
//! fails afterwards leaves it describing code the features, the use cases and
//! the rendered docs know nothing about.

use std::collections::BTreeMap;
use std::path::Path;

use retrodoc_ingest::{FileKind, IngestResult};
use serde::{Deserialize, Serialize};

use crate::artifact::{load_json, save_json, Artifact};
use crate::cache::hash_content;
use crate::error::PipelineError;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SourceHashes {
    #[serde(default)]
    files: BTreeMap<String, String>,
}

impl SourceHashes {
    /// Hashes the source files of `ingest` as they are now (lossy UTF-8, like
    /// the repo map). An unreadable file is left out: it is not judged later.
    #[must_use]
    pub fn capture(repo_root: &Path, ingest: &IngestResult) -> Self {
        let files = ingest
            .files
            .iter()
            .filter(|f| f.kind == FileKind::Source)
            .filter_map(|f| {
                let bytes = std::fs::read(repo_root.join(&f.path)).ok()?;
                Some((
                    f.path.to_string_lossy().into_owned(),
                    hash_content(&String::from_utf8_lossy(&bytes)),
                ))
            })
            .collect();
        Self { files }
    }

    /// Hashes `paths` as they are now, the same way as [`Self::capture`].
    #[must_use]
    pub fn capture_paths<'a>(
        repo_root: &Path,
        paths: impl IntoIterator<Item = &'a std::path::PathBuf>,
    ) -> Self {
        let files = paths
            .into_iter()
            .filter_map(|path| {
                let bytes = std::fs::read(repo_root.join(path)).ok()?;
                Some((
                    path.to_string_lossy().into_owned(),
                    hash_content(&String::from_utf8_lossy(&bytes)),
                ))
            })
            .collect();
        Self { files }
    }

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
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_json(&Artifact::SourceHashes.path(repo_root), self)
    }

    #[must_use]
    pub fn get(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    /// Sets the hash of `path`.
    pub fn put(&mut self, path: &str, hash: &str) {
        self.files.insert(path.to_string(), hash.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrodoc_ingest::FileEntry;
    use std::collections::HashMap;

    fn entry(path: &str, kind: FileKind) -> FileEntry {
        FileEntry {
            path: path.into(),
            kind,
            size_bytes: 0,
        }
    }

    #[test]
    fn a_partial_capture_replaces_its_files_and_leaves_the_others() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rb"), "a v1").unwrap();
        std::fs::write(dir.path().join("b.rb"), "b v1").unwrap();
        let paths = |names: &[&str]| -> Vec<std::path::PathBuf> {
            names.iter().map(std::path::PathBuf::from).collect()
        };
        let mut recorded = SourceHashes::capture_paths(dir.path(), &paths(&["a.rb", "b.rb"]));
        let b_before = recorded.get("b.rb").unwrap().to_string();

        // Both files change, a later run documents `a.rb` only.
        std::fs::write(dir.path().join("a.rb"), "a v2").unwrap();
        std::fs::write(dir.path().join("b.rb"), "b v2").unwrap();
        recorded.merge(SourceHashes::capture_paths(dir.path(), &paths(&["a.rb"])));

        assert_eq!(recorded.get("a.rb"), Some(hash_content("a v2").as_str()));
        assert_eq!(
            recorded.get("b.rb"),
            Some(b_before.as_str()),
            "b was not documented again"
        );
    }

    #[test]
    fn captures_the_source_files_only_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rb"), "code").unwrap();
        std::fs::write(dir.path().join("README.md"), "doc").unwrap();
        let ingest = IngestResult {
            files: vec![
                entry("a.rb", FileKind::Source),
                entry("README.md", FileKind::Markdown),
                entry("gone.rb", FileKind::Source),
            ],
            history_by_path: HashMap::new(),
            existing_docs: vec![],
            commits: vec![],
        };
        SourceHashes::capture(dir.path(), &ingest)
            .save(dir.path())
            .unwrap();
        let loaded = SourceHashes::load(dir.path());
        assert_eq!(loaded.get("a.rb"), Some(hash_content("code").as_str()));
        assert_eq!(loaded.get("README.md"), None);
        assert_eq!(loaded.get("gone.rb"), None);
    }

    #[test]
    fn nothing_saved_means_no_hash() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(SourceHashes::load(dir.path()).get("a.rb"), None);
    }
}
