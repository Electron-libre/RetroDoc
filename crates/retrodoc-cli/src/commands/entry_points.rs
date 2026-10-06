use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{FileRole, RoleRules};

/// Reads the entry points (routes, commands, jobs, public API…) and their
/// outputs from the files classified `entrypoint` (using the rules from
/// `retrodoc roles`), saves `.retrodoc/cache/entry-points.yaml` and prints a
/// summary. Unchanged files are not sent to the LLM again.
pub async fn run(path: &Path, tracker: &UsageTracker) -> anyhow::Result<()> {
    let repo_root = path
        .canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))?;
    let config = Config::load(&repo_root)
        .with_context(|| "config not found — run `retrodoc init` first".to_string())?;
    let rules = RoleRules::load(&repo_root)
        .context("no file role rules found — run `retrodoc roles` first")?;
    let ingest = retrodoc_ingest::run(&repo_root, &config.ingest)
        .with_context(|| format!("ingestion of {} failed", repo_root.display()))?;
    let role_map = rules.classify(&ingest.files);
    let files = role_map.files_with(FileRole::EntryPoint).len();

    let llm = super::usage::provider(&config.llm, tracker)?;
    tracker.set_pass("entry-points");
    println!("Reading entry points from {files} file(s)…");
    let inventory = retrodoc_pipeline::build_entry_points(&repo_root, &role_map, &llm)
        .await
        .context("failed to build the entry points inventory")?;

    println!("\nEntry points:");
    for (kind, count) in inventory.distribution() {
        println!("  {:<12} {count}", kind.label());
    }
    println!();
    for (file, entry) in inventory.iter() {
        let outputs = entry
            .outputs
            .iter()
            .map(|o| o.description.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        println!(
            "  [{}] {} ({}) — {}{}",
            entry.kind.label(),
            entry.name,
            file.display(),
            entry.description,
            if outputs.is_empty() {
                String::new()
            } else {
                format!(" → {outputs}")
            }
        );
    }
    println!("\nSaved to .retrodoc/cache/entry-points.yaml.");
    Ok(())
}
