//! The start of `generate`: the repo opened, the format of its cache checked, the human corrections read, the
//! cache made ready for `--force`, the repo walked.

use std::path::Path;

use anyhow::Context;
use retrodoc_core::cache::format::{self as cache_format, FormatError};
use retrodoc_ingest::IngestResult;
use retrodoc_pipeline::Corrections;

use super::force;
use crate::commands::workspace::Workspace;

/// Opens the repo, reads the human corrections (the estimate included), wipes the caches when asked (`force`, unless the run only `estimate`s), and walks
/// it without the docs `generate` writes. Refuses a cache of another format first, the estimate
/// included (it would count the units of a run that is refused): an older one unless `force`
/// rewrites it (a rerun would keep its saved units), a later or unreadable one always (CFV-4, CFV-5).
pub(super) fn open_workspace(
    path: &Path,
    force: bool,
    estimate: bool,
) -> anyhow::Result<(Workspace, IngestResult, Corrections)> {
    let workspace = Workspace::open(path)?;
    match cache_format::check(&workspace.repo_root) {
        Err(FormatError::Older { .. }) if force => {}
        checked => checked.context("nothing was run")?,
    }
    // The render refuses a file of edits it can't read: refused before any LLM call is paid for,
    // and read before `force` moves or removes the saved sections they correct. The estimate reads
    // them too: it counts the units a review asks again, and a run they would refuse (EST-14).
    let corrections = super::corrections::load(&workspace.repo_root)?;
    if force && !estimate {
        force::set_aside_older(&workspace.repo_root)?;
        force::clear_caches(&workspace.repo_root)?;
    }
    let mut ingest = workspace.ingest()?;
    ingest.existing_docs =
        crate::commands::docs::without_generated(ingest.existing_docs, &workspace.config);
    Ok((workspace, ingest, corrections))
}
