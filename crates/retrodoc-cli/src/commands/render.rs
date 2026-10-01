use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;

/// Writes (or, with `dry_run`, previews) the docs from the artifacts of the
/// last `generate` run; makes no LLM call.
pub fn run(path: &Path, dry_run: bool) -> anyhow::Result<()> {
    let repo_root = path
        .canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))?;
    let config = Config::load(&repo_root)
        .with_context(|| "config not found — run `retrodoc init` first".to_string())?;
    super::docs::publish(&repo_root, &config, dry_run)
}
