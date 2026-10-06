use std::path::Path;

use super::workspace::Workspace;

/// Writes (or, with `dry_run`, previews) the docs from the artifacts of the
/// last `generate` run; makes no LLM call.
pub fn run(path: &Path, dry_run: bool) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    super::docs::publish(&workspace.repo_root, &workspace.config, dry_run)
}
