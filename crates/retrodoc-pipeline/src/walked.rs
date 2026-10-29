//! The files of the walk of this run: the only ones a path taken from a saved artifact may read. An artifact is a
//! file a user or another tool can edit, so a path it cites could name a file the walker skips (a gitignored `.env`,
//! `.git/config`) and send its content to the LLM; read through [`WalkedFiles::read`], such a path is unreadable.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use retrodoc_ingest::walker::{walk_repo, FileEntry};

use crate::error::PipelineError;
use crate::text::{read_file_lossy, read_file_prefix_lossy};

/// The repository root and the relative paths of the files its walk yields. The default holds no file: nothing
/// is readable through it.
#[derive(Debug, Clone, Default)]
pub struct WalkedFiles {
    root: PathBuf,
    files: BTreeSet<PathBuf>,
}

impl WalkedFiles {
    /// The files of `entries`, the walk of `root`. Taken before the file budget, which leaves files out of the
    /// passes but not out of the repository.
    #[must_use]
    pub fn new(root: &Path, entries: &[FileEntry]) -> Self {
        Self {
            root: root.to_path_buf(),
            files: entries.iter().map(|entry| entry.path.clone()).collect(),
        }
    }

    /// Walks `root` as the ingestion does, `extra_ignore` included.
    ///
    /// # Errors
    ///
    /// Returns an error if the walk fails.
    pub fn walk(root: &Path, extra_ignore: &[String]) -> Result<Self, PipelineError> {
        let entries = walk_repo(root, extra_ignore).map_err(|error| PipelineError::Read {
            path: root.to_path_buf(),
            source: std::io::Error::other(error.to_string()),
        })?;
        Ok(Self::new(root, &entries))
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether `relative` (a leading `./` allowed) is a file of the walk.
    #[must_use]
    pub fn contains(&self, relative: &Path) -> bool {
        let plain: PathBuf = relative
            .components()
            .filter(|component| !matches!(component, Component::CurDir))
            .collect();
        self.files.contains(&plain)
    }

    /// The file at `relative` as text, as `read_file_lossy` reads it, refused as unreadable when it is not a file of
    /// the walk.
    pub(crate) fn read(&self, relative: &Path) -> Result<String, PipelineError> {
        self.check(relative)?;
        read_file_lossy(&self.root, relative)
    }

    /// The first `max_bytes` bytes of the file at `relative`, as `read_file_prefix_lossy` reads them, refused as
    /// [`Self::read`] refuses.
    pub(crate) fn read_prefix(
        &self,
        relative: &Path,
        max_bytes: usize,
    ) -> Result<String, PipelineError> {
        self.check(relative)?;
        read_file_prefix_lossy(&self.root, relative, max_bytes)
    }

    /// An error when `relative` is not a file of the walk.
    fn check(&self, relative: &Path) -> Result<(), PipelineError> {
        if self.contains(relative) {
            Ok(())
        } else {
            Err(PipelineError::Read {
                path: relative.to_path_buf(),
                source: std::io::Error::new(ErrorKind::NotFound, "not a file of the walk"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walked_file_is_read_and_another_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rb"), "walked").unwrap();
        let walked = WalkedFiles::walk(dir.path(), &[]).unwrap();
        std::fs::write(dir.path().join("late.rb"), "not walked").unwrap();
        assert_eq!(walked.read(Path::new("a.rb")).unwrap(), "walked");
        assert_eq!(walked.read(Path::new("./a.rb")).unwrap(), "walked");
        assert!(walked.read(Path::new("late.rb")).is_err());
    }
}
