//! The snapshot of a finished `generate` (`specs/run-history.md`, ADR 0043): the docs it left,
//! read back as `retrodoc ui` reads them, saved in the run history.

use std::path::Path;

use retrodoc_ingest::git_history::head_commit;
use retrodoc_mcp::history;
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::usage_log::RunUsage;

use super::docs::project_docs;
use super::workspace::{repo_root, Workspace};

/// The commit of `HEAD` when a run starts, `None` with no commit.
pub struct Head(Option<String>);

/// The commit of `HEAD` of the repository at `path`, read before the run.
#[must_use]
pub fn head(path: &Path) -> Head {
    Head(repo_root(path).ok().and_then(|root| head_commit(&root)))
}

/// Saves the snapshot of the `generate` run on `path` that read `head` and cost `usage` (`None`:
/// no LLM call). Best effort: a failure is warned of, it never fails the finished command.
pub fn record(path: &Path, head: Head, usage: Option<&RunUsage>) {
    if let Err(error) = save(path, head, usage) {
        tracing::warn!("could not save the snapshot of the run: {error:#}");
    }
}

fn save(path: &Path, Head(commit): Head, usage: Option<&RunUsage>) -> anyhow::Result<()> {
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
    let snapshot = history::snapshot(&generated.reconciled(&docs), commit, &finished_at, usage);
    if let Some(file) = history::save(root, &snapshot)? {
        tracing::info!("snapshot of the run saved in {}", file.display());
    }
    Ok(())
}
