use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_ingest::{FileKind, IngestResult};
use retrodoc_llm::OpenRouterProvider;
use retrodoc_pipeline::RepoMap;

/// Current pipeline stage (PLAN.md §5, "repo map" phase): ingestion +
/// bottom-up repo map. The following steps (domains, features, use cases,
/// diagrams, confidence, writing to `docs/`) arrive in later roadmap phases.
pub async fn run(path: &Path) -> anyhow::Result<()> {
    let repo_root = path
        .canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))?;

    let config = Config::load(&repo_root)
        .with_context(|| "config not found — run `retrodoc init` first".to_string())?;

    let ingest = retrodoc_ingest::run(&repo_root, &config.ingest)
        .with_context(|| format!("ingestion of {} failed", repo_root.display()))?;

    let llm = OpenRouterProvider::from_config(&config.llm)
        .context("could not initialize the LLM provider (missing API key?)")?;

    println!(
        "Building the repo map ({} source file(s) to summarize)…",
        source_file_count(&ingest)
    );

    let map = retrodoc_pipeline::build_repo_map(&repo_root, &ingest, &llm)
        .await
        .context("failed to build the repo map")?;

    print_repo_map(&map);

    println!(
        "\nRepo map built ({} file(s), {} module(s)), cached in .retrodoc/cache/repo-map.json.",
        map.files.len(),
        map.modules.len()
    );
    println!(
        "Rest of the pipeline (domains, features, use cases, diagrams, confidence, writing) not implemented yet — see PLAN.md §5."
    );

    Ok(())
}

fn source_file_count(ingest: &IngestResult) -> usize {
    ingest
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .count()
}

fn print_repo_map(map: &RepoMap) {
    println!("\nModules:");
    for module in &map.modules {
        let label = if module.path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            module.path.display().to_string()
        };
        println!(
            "  {label} ({} file(s)): {}",
            module.file_count, module.role_summary
        );
    }

    println!("\nFiles:");
    for file in &map.files {
        println!("  {}: {}", file.path.display(), file.role_summary);
    }
}
