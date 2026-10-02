use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_ingest::{FileKind, IngestResult};
use retrodoc_llm::{HeartbeatProvider, LlmProvider, OpenRouterProvider};
use retrodoc_pipeline::{CodeIndex, CoverageReport, DomainMap, EntryPoints, RepoMap, Surface};

/// Current pipeline stage (PLAN.md §5, "confidence score" phase).
///
/// Runs ingestion, the bottom-up repo map, domain/sub-domain clustering,
/// then features and use cases with Mermaid diagrams, and finally the
/// confidence cross-check, and writes the docs (or previews them with
/// `dry_run`). `force` wipes the caches first so nothing is reused.
pub async fn run(path: &Path, dry_run: bool, force: bool) -> anyhow::Result<()> {
    let repo_root = path
        .canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))?;

    let config = Config::load(&repo_root)
        .with_context(|| "config not found — run `retrodoc init` first".to_string())?;

    if force {
        clear_caches(&repo_root)?;
    }

    let mut ingest = retrodoc_ingest::run(&repo_root, &config.ingest)
        .with_context(|| format!("ingestion of {} failed", repo_root.display()))?;
    ingest.existing_docs = super::docs::without_generated(ingest.existing_docs, &config);

    let llm = HeartbeatProvider::new(
        OpenRouterProvider::from_config(&config.llm)
            .context("could not initialize the LLM provider (missing API key?)")?,
    );

    let (surface, entry_points) = build_surface(&repo_root, &ingest, &llm, force).await?;
    println!("Identifying the business actors…");
    let source_files: Vec<_> = ingest
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path.clone())
        .collect();
    let actors = retrodoc_pipeline::build_actors(&repo_root, &source_files, &surface, &llm, force)
        .await
        .context("failed to identify the actors")?;
    println!("{} actor(s) identified.", actors.actors.len());

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
        retrodoc_pipeline::build_domains(&repo_root, &map, &ingest.existing_docs, &surface, &llm)
            .await
            .context("failed to build the domain clustering")?;

    print_domain_map(&domain_map);
    print_coverage_report(&coverage);

    println!("\nDomains saved to .retrodoc/cache/domains.yaml.");

    println!("\nDeriving features…");
    let features = retrodoc_pipeline::build_features(&repo_root, &domain_map, &map, &llm)
        .await
        .context("failed to derive the features")?;

    println!(
        "Deriving use cases ({} feature(s), one LLM call each)…",
        features.len()
    );
    let code_index = CodeIndex::new(
        ingest
            .files
            .iter()
            .filter(|f| f.kind == FileKind::Source)
            .map(|f| f.path.as_path()),
    );
    let mut use_cases = retrodoc_pipeline::build_use_cases(
        &repo_root,
        &features,
        &entry_points,
        &code_index,
        &actors,
        &llm,
    )
    .await
    .context("failed to derive the use cases")?;
    retrodoc_pipeline::attach_diagrams(&mut use_cases);
    // Persist again now that the diagrams are attached.
    retrodoc_pipeline::save_use_cases(&repo_root, &use_cases)
        .context("failed to save the use cases")?;

    let mut features = features;
    println!("\nCross-checking use cases against the code (confidence)…");
    retrodoc_pipeline::score_confidence(&repo_root, &mut features, &mut use_cases, &llm)
        .await
        .context("failed to score the confidence")?;

    print_features(&features, &use_cases);

    println!(
        "\n{} feature(s) saved to .retrodoc/cache/features.yaml, {} use case(s) to .retrodoc/cache/use-cases.yaml.",
        features.len(),
        use_cases.len()
    );
    println!("Run `retrodoc report` for the documentation debt report.");

    super::docs::publish(&repo_root, &config, dry_run)
}

/// Identifies the file roles, then reads the entities and the entry points
/// from the files of those roles: the application surface the domains are
/// clustered from. Each pass is incremental. If no role rules can be
/// identified the surface is empty and the domains fall back to the
/// directory summaries alone.
async fn build_surface(
    repo_root: &Path,
    ingest: &IngestResult,
    llm: &dyn LlmProvider,
    force: bool,
) -> anyhow::Result<(Surface, EntryPoints)> {
    println!("Identifying the stack and the file roles…");
    let rules = retrodoc_pipeline::identify_roles(repo_root, ingest, llm, force)
        .await
        .context("failed to identify the file roles")?;
    if rules.rules.is_empty() {
        tracing::warn!("no file role rules identified, domains are clustered without the surface");
        return Ok((Surface::default(), EntryPoints::default()));
    }
    let role_map = rules.classify(&ingest.files);
    println!("Stack: {}", rules.stack);

    println!("Reading the business entities…");
    let glossary = retrodoc_pipeline::build_glossary(repo_root, &role_map, llm)
        .await
        .context("failed to build the glossary")?;
    println!("Reading the entry points…");
    let entry_points = retrodoc_pipeline::build_entry_points(repo_root, &role_map, llm)
        .await
        .context("failed to build the entry points inventory")?;

    let surface = Surface::new(&glossary, &entry_points);
    println!(
        "Surface: {} entit(ies), {} resource(s) with entry points.",
        surface.entities.len(),
        surface.resources.len()
    );
    Ok((surface, entry_points))
}

/// Removes the cached results of the LLM passes (not `domains.yaml`, which
/// is recomputed on every run anyway, nor the hand-editable `roles.yaml`;
/// `retrodoc roles --force` re-identifies the latter).
fn clear_caches(repo_root: &Path) -> anyhow::Result<()> {
    for name in [
        "repo-map.json",
        "glossary.yaml",
        "entry-points.yaml",
        "actors.yaml",
        "fingerprints.json",
        "features.yaml",
        "use-cases.yaml",
    ] {
        let file = repo_root.join(".retrodoc/cache").join(name);
        match std::fs::remove_file(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| format!("could not remove {}", file.display()))
            }
        }
    }
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

fn print_features(features: &[Feature], use_cases: &[UseCase]) {
    println!("\nFeatures:");
    for feature in features {
        println!(
            "  {}/{} — {} [{}]",
            feature.domain_slug,
            feature.slug,
            feature.name,
            confidence_label(feature.confidence.as_ref())
        );
        for use_case in use_cases.iter().filter(|u| u.feature_slug == feature.slug) {
            println!(
                "    - {} ({} step(s)) [{}]",
                use_case.name,
                use_case.steps.len(),
                confidence_label(use_case.confidence.as_ref())
            );
        }
    }
}

fn confidence_label(score: Option<&ConfidenceScore>) -> String {
    score.map_or_else(
        || "not scored".to_string(),
        |c| format!("{:.0}%", c.value * 100.0),
    )
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
