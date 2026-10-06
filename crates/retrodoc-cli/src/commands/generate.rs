use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::{Config, LlmConfig, DEFAULT_BATCH_CHARS};
use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_ingest::{FileKind, IngestResult};
use retrodoc_llm::{LlmProvider, UsageTracker};
use retrodoc_pipeline::{
    Actors, CodeIndex, CoverageReport, DomainMap, EntryPoints, RepoMap, RepoMapOptions, RoleMap,
    Scope, Surface, UseCaseContext,
};

/// Number of main entity names given to the use cases as business vocabulary.
const VOCABULARY_SIZE: usize = 40;

/// What the confidence pass does in this run.
#[derive(Debug, Clone, Copy)]
pub enum Confidence {
    /// Not run at all.
    Skip,
    /// Run, on at most this many unscored use cases (`None`: all of them).
    Sample(Option<usize>),
}

/// Current pipeline stage (PLAN.md §5, "confidence score" phase).
///
/// Runs ingestion, the bottom-up repo map, domain/sub-domain clustering,
/// then features and use cases with Mermaid diagrams, and finally the
/// confidence cross-check, and writes the docs (or previews them with
/// `dry_run`). `force` wipes the caches first so nothing is reused.
pub async fn run(
    path: &Path,
    dry_run: bool,
    force: bool,
    confidence: Confidence,
    max_files: Option<usize>,
    tracker: &UsageTracker,
) -> anyhow::Result<()> {
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

    let llm = super::usage::provider(&config.llm, tracker)?;

    let (surface, entry_points, role_map) =
        build_surface(&repo_root, &mut ingest, &llm, force, tracker).await?;
    tracker.set_pass("actors");
    let actors = identify_actors(&repo_root, &ingest, &surface, &llm, force).await?;

    // Ranking the files is not an LLM pass either.
    tracker.end_pass();
    apply_scope(
        &repo_root,
        &mut ingest,
        role_map.as_ref(),
        max_files.or(config.ingest.max_files),
    )?;

    tracker.set_pass("repo-map");
    let map = build_map(&repo_root, &ingest, &llm, &config.llm).await?;

    print_repo_map(&map);

    println!(
        "\nRepo map built ({} file(s), {} module(s)), cached in .retrodoc/cache/repo-map.json.",
        map.files.len(),
        map.modules.len()
    );

    tracker.set_pass("domains");
    let domain_map = cluster_domains(&repo_root, &map, &ingest, &surface, &llm).await?;

    tracker.set_pass("features");
    println!("\nDeriving features…");
    let features = retrodoc_pipeline::build_features(&repo_root, &domain_map, &map, &llm)
        .await
        .context("failed to derive the features")?;

    tracker.set_pass("use-cases");
    let mut use_cases = derive_use_cases(
        &repo_root,
        &ingest,
        &features,
        entry_points,
        actors,
        &surface,
        &llm,
    )
    .await?;

    let mut features = features;
    tracker.set_pass("confidence");
    run_confidence(&repo_root, &mut features, &mut use_cases, &llm, confidence).await?;

    // Rendering is not an LLM pass: don't bill its time to the last one.
    tracker.end_pass();
    print_features(&features, &use_cases);

    println!(
        "\n{} feature(s) saved to .retrodoc/cache/features.yaml, {} use case(s) to .retrodoc/cache/use-cases.yaml.",
        features.len(),
        use_cases.len()
    );
    println!("Run `retrodoc report` for the documentation debt report.");

    super::docs::publish(&repo_root, &config, dry_run)
}

fn source_paths(ingest: &IngestResult) -> impl Iterator<Item = &Path> {
    ingest
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path.as_path())
}

async fn identify_actors(
    repo_root: &Path,
    ingest: &IngestResult,
    surface: &Surface,
    llm: &dyn LlmProvider,
    force: bool,
) -> anyhow::Result<Actors> {
    println!("Identifying the business actors…");
    let source_files: Vec<_> = source_paths(ingest).map(Path::to_path_buf).collect();
    let actors = retrodoc_pipeline::build_actors(repo_root, &source_files, surface, llm, force)
        .await
        .context("failed to identify the actors")?;
    println!("{} actor(s) identified.", actors.actors.len());
    Ok(actors)
}

async fn cluster_domains(
    repo_root: &Path,
    map: &RepoMap,
    ingest: &IngestResult,
    surface: &Surface,
    llm: &dyn LlmProvider,
) -> anyhow::Result<DomainMap> {
    println!("\nClustering into functional domains…");
    let (domain_map, coverage) =
        retrodoc_pipeline::build_domains(repo_root, map, &ingest.existing_docs, surface, llm)
            .await
            .context("failed to build the domain clustering")?;

    print_domain_map(&domain_map);
    print_coverage_report(&coverage);

    println!("\nDomains saved to .retrodoc/cache/domains.yaml.");
    Ok(domain_map)
}

/// Use cases of the features, with their diagrams and business-language
/// score, saved.
async fn derive_use_cases(
    repo_root: &Path,
    ingest: &IngestResult,
    features: &[Feature],
    entry_points: EntryPoints,
    actors: Actors,
    surface: &Surface,
    llm: &dyn LlmProvider,
) -> anyhow::Result<Vec<UseCase>> {
    println!(
        "Deriving use cases ({} feature(s), one LLM call each)…",
        features.len()
    );
    let context = UseCaseContext {
        entry_points,
        index: CodeIndex::new(source_paths(ingest)),
        actors,
        vocabulary: surface.vocabulary(VOCABULARY_SIZE),
    };
    let mut use_cases = retrodoc_pipeline::build_use_cases(repo_root, features, &context, llm)
        .await
        .context("failed to derive the use cases")?;
    retrodoc_pipeline::attach_diagrams(&mut use_cases);
    // Deterministic, so recomputed every run: does each use case read as business?
    retrodoc_pipeline::score_business_language(
        &mut use_cases,
        &surface.vocabulary(usize::MAX),
        &context.actors,
    );
    // Persist again now that the diagrams are attached.
    retrodoc_pipeline::save_use_cases(repo_root, &use_cases)
        .context("failed to save the use cases")?;
    Ok(use_cases)
}

/// Identifies the file roles, then reads the entities and the entry points
/// from the files of those roles: the application surface the domains are
/// clustered from. Each pass is incremental. If no role rules can be
/// identified the surface is empty and the domains fall back to the
/// directory summaries alone.
async fn build_surface(
    repo_root: &Path,
    ingest: &mut IngestResult,
    llm: &dyn LlmProvider,
    force: bool,
    tracker: &UsageTracker,
) -> anyhow::Result<(Surface, EntryPoints, Option<RoleMap>)> {
    tracker.set_pass("roles");
    println!("Identifying the stack and the file roles…");
    let rules = retrodoc_pipeline::identify_roles(repo_root, ingest, llm, force)
        .await
        .context("failed to identify the file roles")?;
    let promoted = rules.promote_sources(&mut ingest.files);
    if promoted > 0 {
        println!("{promoted} file(s) of the identified languages added to the source files.");
    }
    if rules.rules.is_empty() {
        tracing::warn!("no file role rules identified, domains are clustered without the surface");
        return Ok((Surface::default(), EntryPoints::default(), None));
    }
    let role_map = rules.classify(&ingest.files);
    println!("Stack: {}", rules.stack);

    tracker.set_pass("glossary");
    println!("Reading the business entities…");
    let glossary = retrodoc_pipeline::build_glossary(repo_root, &role_map, llm)
        .await
        .context("failed to build the glossary")?;
    tracker.set_pass("entry-points");
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
    Ok((surface, entry_points, Some(role_map)))
}

/// Keeps the best ranked source files when a budget is set (the others
/// leave `ingest` and are saved in `scope.yaml` for the report).
fn apply_scope(
    repo_root: &Path,
    ingest: &mut IngestResult,
    roles: Option<&RoleMap>,
    max_files: Option<usize>,
) -> anyhow::Result<()> {
    let Some(max_files) = max_files else {
        Scope::clear(repo_root);
        return Ok(());
    };
    let scope = retrodoc_pipeline::apply_budget(repo_root, ingest, roles, max_files);
    scope.save(repo_root).context("failed to save the scope")?;
    if scope.skipped.is_empty() {
        println!("Budget of {max_files} file(s): every source file is analysed.");
    } else {
        println!(
            "Budget of {max_files} file(s): {} analysed, {} left out (best ranked by role, history and references first; listed in the report).",
            scope.analysed,
            scope.skipped.len()
        );
    }
    Ok(())
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

async fn run_confidence(
    repo_root: &Path,
    features: &mut [Feature],
    use_cases: &mut [UseCase],
    llm: &dyn LlmProvider,
    confidence: Confidence,
) -> anyhow::Result<()> {
    match confidence {
        Confidence::Sample(sample) => {
            println!("\nCross-checking use cases against the code (confidence)…");
            retrodoc_pipeline::score_confidence(repo_root, features, use_cases, llm, sample)
                .await
                .context("failed to score the confidence")
        }
        Confidence::Skip => {
            println!(
                "\nConfidence pass skipped (--no-confidence): unscored use cases stay unscored."
            );
            Ok(())
        }
    }
}

/// Sizes the repo map pass from the caches, then runs it.
async fn build_map(
    repo_root: &Path,
    ingest: &IngestResult,
    llm: &dyn LlmProvider,
    config: &LlmConfig,
) -> anyhow::Result<RepoMap> {
    let options = RepoMapOptions {
        concurrency: config.concurrency.unwrap_or(1),
        batch_chars: config.batch_chars.unwrap_or(DEFAULT_BATCH_CHARS),
    };
    let estimate = retrodoc_pipeline::estimate_repo_map(repo_root, ingest, options.batch_chars);
    println!(
        "Building the repo map: {} source file(s), {} file call(s) + {} directory call(s) expected \
         (~{}k chars to send; the later passes cost about one call per domain unit, feature and use case).",
        estimate.files,
        estimate.file_calls,
        estimate.directories_to_summarize,
        estimate.chars_to_send / 1000
    );
    retrodoc_pipeline::build_repo_map(repo_root, ingest, llm, options)
        .await
        .context("failed to build the repo map")
}
