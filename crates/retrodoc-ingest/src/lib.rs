//! `retrodoc-ingest`: basic ingestion of the analyzed repo (PLAN.md §2 step 1).
//!
//! Combines the file walk (respecting `.gitignore`), the per-file Git
//! history, and the loading of existing Markdown docs. The result is the
//! basis for the "repo map" (next roadmap phase).

pub mod error;
pub mod existing_docs;
pub mod git_history;
pub mod signals;
pub mod walker;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use error::IngestError;
pub use existing_docs::ExistingDoc;
pub use git_history::{CommitSubject, FileHistory};
pub use signals::{Signal, SignalKind};
pub use walker::{promote_to_source, FileEntry, FileKind};

use retrodoc_core::config::IngestConfig;

#[derive(Debug, Clone)]
pub struct IngestResult {
    pub files: Vec<FileEntry>,
    pub history_by_path: HashMap<PathBuf, FileHistory>,
    pub existing_docs: Vec<ExistingDoc>,
    /// Subjects of the non-merge commits, newest first (read by the same walk
    /// as `history_by_path`).
    pub commits: Vec<CommitSubject>,
}

impl IngestResult {
    #[must_use]
    pub fn history_for(&self, path: &Path) -> Option<&FileHistory> {
        self.history_by_path.get(path)
    }
}

/// Runs the full ingestion (files + git history + existing docs) on the
/// repo located at `repo_root`.
///
/// # Errors
///
/// Returns an error if walking the repo, reading the git history, or
/// loading existing Markdown docs fails.
pub fn run(repo_root: &Path, config: &IngestConfig) -> Result<IngestResult, IngestError> {
    let files = walker::walk_repo(repo_root, &config.extra_ignore)?;
    let git_history::HistoryScan {
        files: history_by_path,
        commits,
    } = git_history::scan_history(repo_root)?;
    let existing_docs = existing_docs::load_existing_docs(repo_root, &config.existing_docs_paths)?;

    Ok(IngestResult {
        files,
        history_by_path,
        existing_docs,
        commits,
    })
}
