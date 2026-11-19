//! What the budget on the number of source files kept and left out, saved as
//! `.retrodoc/cache/scope.yaml` for the debt report.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache::{load_yaml, save_yaml, Artifact, CacheError};

/// What the budget kept and left out, as saved in `scope.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    /// The budget of this run.
    pub max_files: usize,
    /// Source files analysed.
    pub analysed: usize,
    /// Source files left out, in path order.
    pub skipped: Vec<PathBuf>,
}

impl Scope {
    /// Missing or unreadable: `None`.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Scope.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the file can't be written.
    pub fn save(&self, repo_root: &Path) -> Result<(), CacheError> {
        save_yaml(&Artifact::Scope.path(repo_root), self)
    }

    /// Removes the saved scope (no budget in this run).
    pub fn clear(repo_root: &Path) {
        let _ = std::fs::remove_file(Artifact::Scope.path(repo_root));
    }
}
