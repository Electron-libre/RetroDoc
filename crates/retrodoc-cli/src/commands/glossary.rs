use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_llm::{HeartbeatProvider, OpenRouterProvider};
use retrodoc_pipeline::{FileRole, RoleRules};

/// Reads the business entities of the files classified `model` (using the
/// rules from `retrodoc roles`) and the vocabulary of the tests, saves
/// `.retrodoc/cache/glossary.yaml` and prints a summary. Unchanged model
/// files are not sent to the LLM again.
pub async fn run(path: &Path) -> anyhow::Result<()> {
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
    let model_files = role_map.files_with(FileRole::Model).len();

    let llm = HeartbeatProvider::new(
        OpenRouterProvider::from_config(&config.llm)
            .context("could not initialize the LLM provider (missing API key?)")?,
    );
    println!("Reading entities from {model_files} model file(s)…");
    let glossary = retrodoc_pipeline::build_glossary(&repo_root, &role_map, &llm)
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
    println!("Saved to .retrodoc/cache/glossary.yaml.");
    Ok(())
}
