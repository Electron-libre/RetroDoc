use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::{Config, CONFIG_FILE_NAME};

pub fn run(path: &Path, force: bool) -> anyhow::Result<()> {
    let repo_root = path
        .canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))?;

    Config::write_default(&repo_root, force).with_context(|| {
        format!(
            "could not write {} into {}",
            CONFIG_FILE_NAME,
            repo_root.display()
        )
    })?;

    println!(
        "{} created in {}. Set the OPENROUTER_API_KEY environment variable before `retrodoc generate`.",
        CONFIG_FILE_NAME,
        repo_root.display()
    );
    Ok(())
}
