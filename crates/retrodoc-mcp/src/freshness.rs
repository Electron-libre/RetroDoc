//! Whether the code the docs cite is still the code they were generated from:
//! the current hash of each cited file against the one the last `generate`
//! recorded in `repo-map.json`, checked at every call so a long-running
//! server notices an edit made meanwhile.

use std::path::{Path, PathBuf};

use retrodoc_pipeline::cache::{hash_content, RepoMapCache};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    /// Same content as when the docs were generated.
    Fresh,
    Modified,
    Deleted,
    /// No hash was recorded (a doc, a file left out by the file budget): not judged.
    Unknown,
}

pub struct Freshness {
    repo_root: PathBuf,
    recorded: RepoMapCache,
}

impl Freshness {
    /// Reads the hashes recorded by the last `generate`; with none (never
    /// generated) every file is [`FileState::Unknown`].
    #[must_use]
    pub fn load(repo_root: &Path) -> Self {
        Self {
            repo_root: repo_root.to_path_buf(),
            recorded: RepoMapCache::load(repo_root),
        }
    }

    #[must_use]
    pub fn state(&self, path: &str) -> FileState {
        let Some(recorded) = self.recorded.content_hash(Path::new(path)) else {
            return FileState::Unknown;
        };
        match std::fs::read(self.repo_root.join(path)) {
            // Hashed as the repo map does: lossy UTF-8.
            Ok(bytes) if hash_content(&String::from_utf8_lossy(&bytes)) == recorded => {
                FileState::Fresh
            }
            Ok(_) => FileState::Modified,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileState::Deleted,
            Err(_) => FileState::Unknown,
        }
    }

    /// The files among `paths` that are modified or deleted, in order, with
    /// their state.
    #[must_use]
    pub fn stale<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> Vec<(&'a str, FileState)> {
        paths
            .into_iter()
            .map(|path| (path, self.state(path)))
            .filter(|(_, state)| matches!(state, FileState::Modified | FileState::Deleted))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repo with `a.rb`, `b.rb`, `c.rb` and the hashes of their content
    /// recorded, as `generate` leaves them.
    fn generated_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = RepoMapCache::default();
        for name in ["a.rb", "b.rb", "c.rb"] {
            let content = format!("content of {name}");
            std::fs::write(dir.path().join(name), &content).unwrap();
            cache.put(Path::new(name), &hash_content(&content), "summary");
        }
        cache.save(dir.path()).unwrap();
        dir
    }

    #[test]
    fn untouched_files_are_fresh() {
        let dir = generated_repo();
        let freshness = Freshness::load(dir.path());
        assert_eq!(freshness.state("a.rb"), FileState::Fresh);
        assert_eq!(freshness.stale(["a.rb", "b.rb"]), vec![]);
    }

    #[test]
    fn an_edit_or_a_deletion_after_generation_is_noticed_at_the_next_call() {
        let dir = generated_repo();
        let freshness = Freshness::load(dir.path());
        std::fs::write(dir.path().join("b.rb"), "edited").unwrap();
        std::fs::remove_file(dir.path().join("c.rb")).unwrap();
        assert_eq!(freshness.state("b.rb"), FileState::Modified);
        assert_eq!(freshness.state("c.rb"), FileState::Deleted);
        assert_eq!(
            freshness.stale(["a.rb", "b.rb", "c.rb"]),
            [("b.rb", FileState::Modified), ("c.rb", FileState::Deleted)]
        );
    }

    #[test]
    fn a_file_without_recorded_hash_is_not_judged() {
        let dir = generated_repo();
        std::fs::write(dir.path().join("README.md"), "docs").unwrap();
        let freshness = Freshness::load(dir.path());
        assert_eq!(freshness.state("README.md"), FileState::Unknown);
        assert_eq!(freshness.state("gone.rb"), FileState::Unknown);
        assert_eq!(freshness.stale(["README.md"]), vec![]);
    }

    #[test]
    fn a_repo_never_generated_has_nothing_to_compare() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rb"), "x").unwrap();
        assert_eq!(
            Freshness::load(dir.path()).state("a.rb"),
            FileState::Unknown
        );
    }
}
