use std::path::Path;

use anyhow::Context;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{Artifact, FileRole, RoleRules};

use super::workspace::Workspace;

/// Reads the entry points (routes, commands, jobs, public API…) and their
/// outputs from the files classified `entrypoint` (using the rules from
/// `retrodoc roles`), saves `.retrodoc/cache/entry-points.yaml` and prints a
/// summary. Unchanged files are not sent to the LLM again.
pub async fn run(path: &Path, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let rules = RoleRules::load(repo_root)
        .context("no file role rules found — run `retrodoc roles` first")?;
    let ingest = workspace.ingest()?;
    let role_map = rules.classify(&ingest.files);
    let files = role_map.files_with(FileRole::EntryPoint).len();

    let llm = super::usage::pass_provider(&config.llm, tracker, "entry-points")?;
    tracker.set_pass("entry-points");
    println!("Reading entry points from {files} file(s)…");
    let inventory = retrodoc_pipeline::build_entry_points(
        repo_root,
        &role_map,
        &super::brief::saved(repo_root),
        llm.as_ref(),
    )
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
    println!("\nSaved to {}.", Artifact::EntryPoints.relative_path());
    Ok(())
}
