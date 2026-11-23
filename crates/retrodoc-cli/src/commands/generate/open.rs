//! The start of `generate`: the repo opened, the format of its cache checked, the edits checked, the
//! cache made ready for `--force`, the repo walked.

use std::path::Path;

use anyhow::Context;
use retrodoc_core::cache::format::{self as cache_format, FormatError};
use retrodoc_ingest::IngestResult;

use super::force;
use crate::commands::workspace::Workspace;

/// Opens the repo, wipes the caches when asked (`force`, unless the run only `estimate`s), and walks
/// it without the docs `generate` writes. Refuses a cache of another format first, the estimate
/// included (it would count the units of a run that is refused): an older one unless `force`
/// rewrites it (a rerun would keep its saved units), a later or unreadable one always (CFV-4, CFV-5).
pub(super) fn open_workspace(
    path: &Path,
    force: bool,
    estimate: bool,
) -> anyhow::Result<(Workspace, IngestResult)> {
    let workspace = Workspace::open(path)?;
    match cache_format::check(&workspace.repo_root) {
        Err(FormatError::Older { .. }) if force => {}
        checked => checked.context("nothing was run")?,
    }
    if !estimate {
        // The render refuses a file of edits it can't read: refused before any LLM call is paid
        // for, and before `force` moves or removes anything.
        retrodoc_mcp::edits::load(&workspace.repo_root)
            .context("nothing was run: the human edits cannot be read")?;
    }
    if force && !estimate {
        force::set_aside_older(&workspace.repo_root)?;
        force::clear_caches(&workspace.repo_root)?;
    }
    let mut ingest = workspace.ingest()?;
    ingest.existing_docs =
        crate::commands::docs::without_generated(ingest.existing_docs, &workspace.config);
    Ok((workspace, ingest))
}
