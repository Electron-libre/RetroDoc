//! The snapshot of a finished `generate` (`specs/run-history.md`, ADR 0043): the docs it left,
//! read back as `retrodoc ui` reads them, saved in the run history.

use std::path::Path;

use retrodoc_ingest::git_history::head_commit;
use retrodoc_mcp::history;
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::usage_log::RunUsage;

use super::docs::project_docs;
use super::workspace::Workspace;

/// Saves the snapshot of the `generate` run on `path` that cost `usage` (`None`: no LLM call).
/// Best effort: a failure is warned of, it never fails the finished command.
pub fn record(path: &Path, usage: Option<&RunUsage>) {
    if let Err(error) = save(path, usage) {
        tracing::warn!("could not save the snapshot of the run: {error:#}");
    }
}

fn save(path: &Path, usage: Option<&RunUsage>) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let root = &workspace.repo_root;
    let docs = project_docs(root, &workspace.config)?;
    let Some(generated) = GeneratedDocs::load(root, &docs) else {
        return Ok(());
    };
    let finished_at = usage.map_or_else(
        || chrono::Utc::now().to_rfc3339(),
        |u| u.finished_at.clone(),
    );
    let snapshot = history::snapshot(
        &generated.reconciled(&docs),
        head_commit(root),
        &finished_at,
        usage,
    );
    if let Some(file) = history::save(root, &snapshot)? {
        tracing::info!("snapshot of the run saved in {}", file.display());
    }
    Ok(())
}
