//! `retrodoc-render`: writes the Markdown/Mermaid output into `docs/`
//! (PLAN.md §3), with a diff preview and idempotence on re-run.
//!
//! The flow is three steps so that nothing touches the disk before the user
//! can look: [`render`] builds the files in memory (deterministic: same
//! input, same bytes), [`plan`] compares them with what is on disk, and
//! [`WritePlan::apply`] writes only the files that actually differ.

mod markdown;
mod plan;

pub use markdown::render;
pub use plan::{plan, FileStatus, PlannedFile, WritePlan};

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Folder of the functional documentation, under the docs dir.
pub const FUNCTIONAL_DIR: &str = "functional";
/// Folder of `RetroDoc`'s own files (coverage report, run metadata), under
/// the docs dir. Neither it nor [`FUNCTIONAL_DIR`] must be fed back as
/// existing documentation on the next run.
pub const META_DIR: &str = "_retrodoc";
/// Path of the index for agents (`llms.txt` style), relative to the docs dir.
pub const AGENT_INDEX_PATH: &str = "functional/llms.txt";
/// Path of the run metadata, relative to the docs dir.
pub const METADATA_PATH: &str = "_retrodoc/run-metadata.json";
/// Path of the coverage report, relative to the docs dir.
pub const COVERAGE_REPORT_PATH: &str = "_retrodoc/coverage-report.md";

/// Which run produced the docs (`_retrodoc/run-metadata.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunMetadata {
    pub model: String,
    /// HEAD of the analyzed repo, if it has a commit.
    pub commit: Option<String>,
    /// RFC 3339 timestamp. Ignored when deciding whether the file changed,
    /// otherwise no run would ever be idempotent.
    pub generated_at: String,
}

/// A file to write, with a path relative to the docs dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedFile {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize the run metadata: {0}")]
    Serialize(#[from] serde_json::Error),
}
