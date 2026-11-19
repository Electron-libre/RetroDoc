//! The capture of the hash of each source file as the docs were produced from it, saved in
//! `source-hashes.json` (`retrodoc_core::artifacts::SourceHashes`) once a `generate` has gone
//! through every pass.
//!
//! The MCP freshness check compares these with the files on disk. The hashes
//! a pass saves can't serve for that: they are updated during a run, so a run
//! that fails afterwards leaves them describing code the features, the use
//! cases and the rendered docs know nothing about.

use std::path::Path;

use retrodoc_core::artifacts::SourceHashes;
use retrodoc_core::cache::hash_content;
use retrodoc_ingest::{FileKind, IngestResult};

use crate::text::read_file_lossy;

/// Hashes the source files of `ingest` as they are now (lossy UTF-8, like
/// the passes read them). An unreadable file is left out: it is not judged later.
#[must_use]
pub fn hash_sources(repo_root: &Path, ingest: &IngestResult) -> SourceHashes {
    ingest
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .filter_map(|f| {
            let content = read_file_lossy(repo_root, &f.path).ok()?;
            Some((
                f.path.to_string_lossy().into_owned(),
                hash_content(&content),
            ))
        })
        .collect()
}

/// Hashes `paths` as they are now, the same way as [`hash_sources`].
#[must_use]
pub fn hash_paths<'a>(
    repo_root: &Path,
    paths: impl IntoIterator<Item = &'a std::path::PathBuf>,
) -> SourceHashes {
    paths
        .into_iter()
        .filter_map(|path| {
            let content = read_file_lossy(repo_root, path).ok()?;
            Some((path.to_string_lossy().into_owned(), hash_content(&content)))
        })
        .collect()
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
        let mut recorded = hash_paths(dir.path(), &paths(&["a.rb", "b.rb"]));
        let b_before = recorded.get("b.rb").unwrap().to_string();

        // Both files change, a later run documents `a.rb` only.
        std::fs::write(dir.path().join("a.rb"), "a v2").unwrap();
        std::fs::write(dir.path().join("b.rb"), "b v2").unwrap();
        recorded.merge(hash_paths(dir.path(), &paths(&["a.rb"])));

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
        hash_sources(dir.path(), &ingest).save(dir.path()).unwrap();
        let loaded = SourceHashes::load(dir.path());
        assert_eq!(loaded.get("a.rb"), Some(hash_content("code").as_str()));
        assert_eq!(loaded.get("README.md"), None);
        assert_eq!(loaded.get("gone.rb"), None);
    }
}
