use std::path::Path;

use anyhow::Context;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{Artifact, BusinessMap, FileRole, RoleRules};

use super::workspace::Workspace;

/// Reads the business entities of the files `retrodoc business-files` located
/// (else those classified `model` by the rules of `retrodoc roles`) and the
/// vocabulary of the tests, saves
/// `.retrodoc/cache/glossary.yaml` and prints a summary. Unchanged model
/// files are not sent to the LLM again.
pub async fn run(path: &Path, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let rules = RoleRules::load(repo_root)
        .context("no file role rules found — run `retrodoc roles` first")?;
    let ingest = workspace.ingest()?;
    let role_map = rules.classify(&ingest.files);
    let business_files = BusinessMap::load(repo_root)
        .map(|map| map.files(&ingest.files))
        .unwrap_or_default();
    let to_read = if business_files.is_empty() {
        role_map.files_with(FileRole::Model).len()
    } else {
        business_files.len()
    };

    let llm = super::usage::pass_provider(&config.llm, tracker, "glossary")?;
    tracker.set_pass("glossary");
    println!("Reading entities from {to_read} file(s)…");
    let glossary = retrodoc_pipeline::build_glossary(
        repo_root,
        &role_map,
        &business_files,
        &super::brief::saved(repo_root),
        llm.as_ref(),
    )
    .await
    .context("failed to build the glossary")?;

    let entities = glossary.merged_entities();
    println!(
        "\nEntities ({} after merging by name, {} per-file entries):",
        entities.len(),
        glossary.entities().count()
    );
    for entity in &entities {
        let associations = entity
            .associations
            .iter()
            .map(|a| format!("{} {}", a.kind, a.target))
            .collect::<Vec<_>>()
            .join(", ");
        let home = entity
            .files
            .first()
            .map_or(String::new(), |f| f.display().to_string());
        let more = entity.files.len().saturating_sub(1);
        println!(
            "  {} ({home}{}): {}{}",
            entity.name,
            if more > 0 {
                format!(" +{more}")
            } else {
                String::new()
            },
            entity.description,
            if associations.is_empty() {
                String::new()
            } else {
                format!(" [{associations}]")
            }
        );
    }
    println!(
        "\nTest vocabulary: {} phrase(s) in {} test file(s).",
        glossary.phrase_count(),
        glossary.tests.len()
    );
    println!("Saved to {}.", Artifact::Glossary.relative_path());
    Ok(())
}
