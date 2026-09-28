use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_ingest::FileKind;

pub fn run(path: &Path) -> anyhow::Result<()> {
    let repo_root = path
        .canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))?;

    let config = Config::load(&repo_root)
        .with_context(|| "config not found — run `retrodoc init` first".to_string())?;

    let result = retrodoc_ingest::run(&repo_root, &config.ingest)
        .with_context(|| format!("ingestion of {} failed", repo_root.display()))?;

    let source_count = result
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .count();
    let markdown_count = result
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Markdown)
        .count();
    let other_count = result.files.len() - source_count - markdown_count;

    println!("Repo: {}", repo_root.display());
    println!(
        "Files: {} (source: {source_count}, markdown: {markdown_count}, other: {other_count})",
        result.files.len()
    );
    println!(
        "Git history: {} file(s) with at least one commit",
        result.history_by_path.len()
    );
    println!(
        "Existing Markdown docs taken as input: {}",
        result.existing_docs.len()
    );
    for doc in &result.existing_docs {
        println!("  - {}", doc.path.display());
    }

    Ok(())
}
