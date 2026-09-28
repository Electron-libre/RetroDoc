use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_ingest::{FileKind, IngestResult};
use retrodoc_llm::OpenRouterProvider;
use retrodoc_pipeline::{CoverageReport, DomainMap, RepoMap};

/// Current pipeline stage (PLAN.md §5, "domains" phase): ingestion +
/// bottom-up repo map + domain/sub-domain clustering. The following steps
/// (features, use cases, diagrams, confidence, writing to `docs/`) arrive in
/// later roadmap phases.
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

    println!("\nClustering into functional domains…");
    let (domain_map, coverage) =
        retrodoc_pipeline::build_domains(&repo_root, &map, &ingest.existing_docs, &llm)
            .await
            .context("failed to build the domain clustering")?;

    print_domain_map(&domain_map);
    print_coverage_report(&coverage);

    println!("\nDomains saved to .retrodoc/cache/domains.yaml.");
    println!(
        "Rest of the pipeline (features, use cases, diagrams, confidence, writing) not implemented yet — see PLAN.md §5."
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

fn print_domain_map(map: &DomainMap) {
    println!("\nDomains:");
    for domain in &map.domains {
        println!(
            "  {} ({}): {} file(s) directly, {} sub-domain(s)",
            domain.name,
            domain.slug,
            domain.paths.len(),
            domain.sub_domains.len()
        );
        for sub in &domain.sub_domains {
            println!(
                "    {} ({}): {} file(s)",
                sub.name,
                sub.slug,
                sub.paths.len()
            );
        }
    }
}

fn print_coverage_report(report: &CoverageReport) {
    if report.is_clean() {
        println!("\nCoverage: 100% of source files assigned, no overlap.");
        return;
    }
    println!("\nCoverage issues found (repaired automatically):");
    if !report.uncovered.is_empty() {
        println!(
            "  {} file(s) unassigned by the LLM, bucketed into \"uncategorized\".",
            report.uncovered.len()
        );
    }
    if !report.overlapping.is_empty() {
        println!(
            "  {} file(s) assigned to more than one domain, kept only the first.",
            report.overlapping.len()
        );
    }
    if !report.unknown.is_empty() {
        println!(
            "  {} unknown path(s) cited by the LLM, dropped.",
            report.unknown.len()
        );
    }
}
