use std::path::Path;

use anyhow::Context;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{Artifact, BusinessMap, RoleRules};

use super::usage::PassProviders;
use super::workspace::Workspace;

/// Reads the files located by `retrodoc business-files` once for both the
/// glossary and the entry points (what `generate` does before those two
/// passes) and saves `glossary.yaml` and `entry-points.yaml`. Unchanged files
/// are not sent to the LLM again.
pub async fn run(path: &Path, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let rules = RoleRules::load(repo_root)
        .context("no file role rules found — run `retrodoc roles` first")?;
    let ingest = workspace.ingest()?;
    let mut role_map = rules.classify(&ingest.files);
    role_map.body_lines = config.pipeline.body_lines;
    let business_files = BusinessMap::load(repo_root)
        .map(|map| map.files(&ingest.files))
        .context("no business files found — run `retrodoc business-files` first")?;

    let llm = PassProviders::open(&config.llm, tracker, &["glossary", "entry-points"])?;
    tracker.set_pass("business-read");
    println!(
        "Reading {} business file(s) for the entities and the entry points…",
        business_files.len()
    );
    retrodoc_pipeline::read_business_files(
        repo_root,
        &role_map,
        &business_files,
        &super::brief::saved(repo_root),
        llm.get("glossary")?.as_ref(),
        llm.get("entry-points")?.as_ref(),
    )
    .await
    .context("failed to read the business files")?;

    println!(
        "\nSaved to {} and {}.",
        Artifact::Glossary.relative_path(),
        Artifact::EntryPoints.relative_path()
    );
    Ok(())
}
