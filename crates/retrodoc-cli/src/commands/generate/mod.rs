use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use retrodoc_core::config::{Config, DomainsFrom};
use retrodoc_core::model::{Feature, UseCase};
use retrodoc_ingest::IngestResult;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{
    Actors, Artifact, CodeIndex, DomainMap, EntryPoints, Evidence, ProductBrief, RoleMap, Scope,
    SourceHashes, Surface, UseCaseContext,
};

mod contexts;
mod print;

use super::usage::PassProviders;
use super::workspace::{source_paths, Workspace};

/// Number of main entity names given to the use cases as business vocabulary.
const VOCABULARY_SIZE: usize = 40;

/// The passes `generate` runs, each with its own LLM settings.
const GENERATE_PASSES: &[&str] = &[
    "sources",
    "brief",
    "roles",
    "business-files",
    "contexts",
    "glossary",
    "entry-points",
    "actors",
    "domains",
    "features",
    "use-cases",
    "rules",
    "confidence",
    "reconcile",
];

/// What the confidence pass does in this run.
#[derive(Debug, Clone, Copy)]
pub enum Confidence {
    /// Not run at all.
    Skip,
    /// Run, on at most this many unscored use cases (`None`: all of them).
    Sample(Option<usize>),
}

/// What the user asked of a `generate` run.
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Preview the docs instead of writing them.
    pub dry_run: bool,
    /// Wipe the caches first so nothing is reused.
    pub force: bool,
    pub confidence: Confidence,
    /// Budget of source files (overrides `ingest.max_files`); on the `surface` path the features
    /// still come from every entry point. With `domains_from = "contexts"`:
    /// the files one run reads, a context never being cut.
    pub max_files: Option<usize>,
    /// With `domains_from = "contexts"`: the contexts to document now (empty: the next ones).
    pub contexts: Vec<String>,
}

/// Current pipeline stage (PLAN.md §5, "confidence score" phase).
///
/// Runs ingestion, the surface, the domains (from the surface, ADR 0035;
/// a bounded context at a time with `domains_from = "contexts"`),
/// then features and use cases with Mermaid diagrams, and finally the
/// confidence cross-check, and writes the docs (or previews them with
/// `dry_run`). `force` wipes the caches first so nothing is reused.
pub async fn run(
    path: &Path,
    options: &GenerateOptions,
    tracker: &UsageTracker,
) -> anyhow::Result<()> {
    let GenerateOptions {
        dry_run,
        force,
        confidence,
        max_files,
        ..
    } = *options;
    let (workspace, mut ingest) = open_workspace(path, force)?;
    let (repo_root, config) = (&workspace.repo_root, &workspace.config);

    let llm = PassProviders::open(&config.llm, tracker, GENERATE_PASSES)?;

    let wanted = super::brief::EvidenceFor::of(config);
    let (brief, evidence) =
        super::brief::for_generate(repo_root, &ingest, &llm, wanted, tracker).await?;
    let (evidence, rule_evidence) = (evidence.units, evidence.rules);

    if config.pipeline.domains_from == DomainsFrom::Contexts {
        let signals = Signals {
            brief,
            evidence,
            rule_evidence,
        };
        return contexts::run(&workspace, &llm, tracker, options, signals, ingest).await;
    }

    let (surface, entry_points, role_map) =
        build_surface(repo_root, &mut ingest, &brief, &llm, force, config, tracker).await?;
    let actors = tracker
        .in_pass(
            "actors",
            identify_actors(repo_root, &ingest, &surface, &brief, &llm, force, false),
        )
        .await?;

    apply_scope(
        repo_root,
        &mut ingest,
        role_map.as_ref(),
        max_files.or(config.ingest.max_files),
    )?;

    // Read before the features: a file edited during the run then reads as
    // modified later, never as fresh.
    let source_hashes = SourceHashes::capture(repo_root, &ingest);

    if let Some(warning) = surface.empty_surface_warning(source_paths(&ingest).count()) {
        tracing::warn!("{warning}");
    }
    let mut features = features_from_surface(
        repo_root,
        &ingest,
        &surface,
        &entry_points,
        &brief,
        &evidence,
        &llm,
        tracker,
    )
    .await?;

    let context = use_case_context(
        entry_points,
        &ingest,
        actors,
        &surface,
        Signals {
            brief,
            evidence,
            rule_evidence,
        },
        config,
    );
    let mut use_cases = tracker
        .in_pass(
            "use-cases",
            derive_use_cases(repo_root, &features, &context, &surface, &llm, Every),
        )
        .await?;

    let passes = Passes {
        repo_root,
        config,
        llm: &llm,
        tracker,
    };
    finish_chain(&passes, &mut features, &mut use_cases, context, confidence).await?;
    // Only a run that went through every pass says which code the docs describe.
    source_hashes
        .save(repo_root)
        .context("could not save the source hashes")?;

    print::features(&features, &use_cases);
    print::saved(features.len(), use_cases.len());

    super::docs::publish(repo_root, config, dry_run)
}

/// Which use cases get their business-language score computed.
#[derive(Debug, Clone, Copy)]
enum Scoring {
    /// All of them, every run: the vocabulary is the whole application's.
    Every,
    /// Those without a score only: read a bounded context at a time, the vocabulary known grows
    /// and the earlier use cases keep the score they were documented with.
    NewOnly,
}

use Scoring::Every;

/// What the passes after the use cases share.
struct Passes<'a> {
    repo_root: &'a Path,
    config: &'a Config,
    llm: &'a PassProviders,
    tracker: &'a UsageTracker,
}

/// The business rules (when on), then the confidence of the use cases.
async fn finish_chain(
    passes: &Passes<'_>,
    features: &mut [Feature],
    use_cases: &mut [UseCase],
    context: UseCaseContext,
    confidence: Confidence,
) -> anyhow::Result<()> {
    let Passes {
        repo_root,
        config,
        llm,
        tracker,
    } = *passes;
    derive_rules(
        repo_root, config, features, use_cases, context, llm, tracker,
    )
    .await?;
    tracker
        .in_pass(
            "confidence",
            run_confidence(
                repo_root,
                features,
                use_cases,
                llm,
                confidence,
                config.llm.for_pass("confidence").concurrency.unwrap_or(1),
            ),
        )
        .await?;
    reconcile_with_docs(repo_root, config, features, use_cases, llm, tracker).await
}

/// Asks whether the doc sections matched to the items cover or contradict them; nothing unless
/// `[pipeline] reconcile` is on. Docs that can't be read leave the verdicts as they were.
async fn reconcile_with_docs(
    repo_root: &Path,
    config: &Config,
    features: &[Feature],
    use_cases: &[UseCase],
    llm: &PassProviders,
    tracker: &UsageTracker,
) -> anyhow::Result<()> {
    use retrodoc_pipeline::reconcile;
    if !config.pipeline.reconcile {
        return Ok(());
    }
    let docs = match super::docs::project_docs(repo_root, config) {
        Ok(docs) => docs,
        Err(err) => {
            tracing::warn!("items not reconciled with the project docs: {err:#}");
            return Ok(());
        }
    };
    let rules = retrodoc_pipeline::load_rules(repo_root).unwrap_or_default();
    let candidates = reconcile::candidates(
        reconcile::items(
            &DomainMap::load(repo_root).unwrap_or_default(),
            features,
            use_cases,
            &rules,
        ),
        &reconcile::corpus(&docs),
    );
    tracker
        .in_pass(
            "reconcile",
            reconcile::judge(
                repo_root,
                llm.get("reconcile")?.as_ref(),
                &config.pipeline,
                &candidates,
            ),
        )
        .await
        .context("failed to reconcile the items with the project docs")
}

/// Opens the repo, wipes the caches when asked, and walks it without the docs `generate` writes.
fn open_workspace(path: &Path, force: bool) -> anyhow::Result<(Workspace, IngestResult)> {
    let workspace = Workspace::open(path)?;
    if force {
        clear_caches(&workspace.repo_root)?;
    }
    let mut ingest = workspace.ingest()?;
    ingest.existing_docs = super::docs::without_generated(ingest.existing_docs, &workspace.config);
    Ok((workspace, ingest))
}

/// The brief and the signals the closest extracts of a feature are taken from.
struct Signals {
    brief: ProductBrief,
    /// For the features and the use cases.
    evidence: Evidence,
    /// For the rules, which need them even when the others are given none.
    rule_evidence: Evidence,
}

/// What the use cases and rules passes draw on besides the features: the vocabulary is the
/// application's, one for every domain.
fn use_case_context(
    entry_points: EntryPoints,
    ingest: &IngestResult,
    actors: Actors,
    surface: &Surface,
    signals: Signals,
    config: &Config,
) -> UseCaseContext {
    let Signals {
        brief,
        evidence,
        rule_evidence,
    } = signals;
    UseCaseContext {
        entry_points,
        index: CodeIndex::new(source_paths(ingest)),
        actors,
        vocabulary: surface.vocabulary(VOCABULARY_SIZE),
        domain_vocabulary: BTreeMap::new(),
        brief,
        evidence,
        rule_evidence,
        put_back: config.rules.clone(),
        concurrency: config.llm.for_pass("use-cases").concurrency.unwrap_or(1),
    }
}

async fn identify_actors(
    repo_root: &Path,
    ingest: &IngestResult,
    surface: &Surface,
    brief: &ProductBrief,
    llm: &PassProviders,
    force: bool,
    frozen: bool,
) -> anyhow::Result<Actors> {
    // Read a slice at a time, the actors would change with every slice and redo the use cases of
    // the others: they are identified once, then kept (`retrodoc actors --force` asks again).
    if let Some(saved) = Actors::load(repo_root).filter(|_| frozen && !force) {
        println!("{} actor(s) kept from the first run.", saved.actors.len());
        return Ok(saved);
    }
    println!("Identifying the business actors…");
    let source_files: Vec<_> = source_paths(ingest).map(Path::to_path_buf).collect();
    let actors = retrodoc_pipeline::build_actors(
        repo_root,
        &source_files,
        surface,
        brief,
        llm.get("actors")?.as_ref(),
        force,
    )
    .await
    .context("failed to identify the actors")?;
    println!("{} actor(s) identified.", actors.actors.len());
    Ok(actors)
}

/// The domains from the entities and resources, then the features from the entry points of each
/// domain.
#[allow(clippy::too_many_arguments)]
async fn features_from_surface(
    repo_root: &Path,
    ingest: &IngestResult,
    surface: &Surface,
    entry_points: &EntryPoints,
    brief: &ProductBrief,
    evidence: &Evidence,
    llm: &PassProviders,
    tracker: &UsageTracker,
) -> anyhow::Result<Vec<Feature>> {
    let domain_map = tracker
        .in_pass(
            "domains",
            cluster_domains_from_surface(repo_root, surface, brief, llm),
        )
        .await?;
    print::surface_domains(&domain_map);

    println!("\nDeriving features from the entry points…");
    let features = tracker
        .in_pass(
            "features",
            retrodoc_pipeline::build_features_from_surface(
                repo_root,
                &domain_map,
                entry_points,
                surface,
                brief,
                evidence,
                llm.get("features")?.as_ref(),
                retrodoc_pipeline::Retention::OnlyRead,
            ),
        )
        .await
        .context("failed to derive the features from the entry points")?;

    let sources: Vec<PathBuf> = source_paths(ingest).map(Path::to_path_buf).collect();
    let coverage = retrodoc_pipeline::save_technical_support(
        repo_root,
        &sources,
        entry_points,
        &features,
        surface,
    )
    .context("could not save the behaviour coverage")?;
    print::behaviour_coverage(&coverage, sources.len());
    Ok(features)
}

async fn cluster_domains_from_surface(
    repo_root: &Path,
    surface: &Surface,
    brief: &ProductBrief,
    llm: &PassProviders,
) -> anyhow::Result<DomainMap> {
    println!("\nClustering the resources and entities into functional domains…");
    retrodoc_pipeline::build_domains_from_surface(
        repo_root,
        surface,
        brief,
        llm.get("domains")?.as_ref(),
    )
    .await
    .context("failed to build the domains from the surface")
}

/// Use cases of the features, with their diagrams and business-language
/// score, saved.
async fn derive_use_cases(
    repo_root: &Path,
    features: &[Feature],
    context: &UseCaseContext,
    surface: &Surface,
    llm: &PassProviders,
    scoring: Scoring,
) -> anyhow::Result<Vec<UseCase>> {
    println!(
        "Deriving use cases ({} feature(s), one LLM call each)…",
        features.len()
    );
    let mut use_cases = retrodoc_pipeline::build_use_cases(
        repo_root,
        features,
        context,
        llm.get("use-cases")?.as_ref(),
    )
    .await
    .context("failed to derive the use cases")?;
    retrodoc_pipeline::attach_diagrams(&mut use_cases);
    // Deterministic: does each use case read as business? Recomputed every run, except read a
    // bounded context at a time, where the vocabulary known grows with every context and the
    // earlier use cases keep the score they were documented with.
    let vocabulary = surface.vocabulary(usize::MAX);
    match scoring {
        Scoring::Every => {
            retrodoc_pipeline::score_business_language(
                &mut use_cases,
                &vocabulary,
                &context.actors,
            );
        }
        Scoring::NewOnly => retrodoc_pipeline::score_unscored_business_language(
            &mut use_cases,
            &vocabulary,
            &context.actors,
        ),
    }
    // Persist again now that the diagrams are attached.
    retrodoc_pipeline::save_use_cases(repo_root, &use_cases)
        .context("failed to save the use cases")?;
    let coverage = retrodoc_pipeline::save_use_case_coverage(repo_root, features, &use_cases)
        .context("could not save the behaviour coverage")?;
    print::use_case_coverage(&coverage, features);
    Ok(use_cases)
}

/// The business rules of the features, saved; nothing unless `[pipeline] rules` is on.
async fn derive_rules(
    repo_root: &Path,
    config: &Config,
    features: &[Feature],
    use_cases: &[UseCase],
    context: UseCaseContext,
    llm: &PassProviders,
    tracker: &UsageTracker,
) -> anyhow::Result<()> {
    if !config.pipeline.rules {
        return Ok(());
    }
    let context = UseCaseContext {
        concurrency: config.llm.for_pass("rules").concurrency.unwrap_or(1),
        ..context
    };
    println!(
        "Stating the business rules ({} feature(s), one LLM call each)…",
        features.len()
    );
    let rules = tracker
        .in_pass(
            "rules",
            retrodoc_pipeline::build_rules(
                repo_root,
                features,
                use_cases,
                &context,
                llm.get("rules")?.as_ref(),
            ),
        )
        .await
        .context("failed to state the business rules")?;
    println!(
        "{} business rule(s) saved to {}.",
        rules.len(),
        Artifact::Rules.relative_path()
    );
    Ok(())
}

/// Identifies the stack and the role of every file (and promotes the files of the languages
/// found to source files). `None` when no role rule could be identified.
async fn identify_file_roles(
    repo_root: &Path,
    ingest: &mut IngestResult,
    brief: &ProductBrief,
    llm: &PassProviders,
    force: bool,
    tracker: &UsageTracker,
) -> anyhow::Result<Option<(String, RoleMap)>> {
    println!("Identifying the stack and the file roles…");
    let rules = tracker
        .in_pass(
            "roles",
            retrodoc_pipeline::identify_roles(
                repo_root,
                ingest,
                brief,
                llm.get("roles")?.as_ref(),
                force,
            ),
        )
        .await
        .context("failed to identify the file roles")?;
    let promoted = rules.promote_sources(&mut ingest.files);
    if promoted > 0 {
        println!("{promoted} file(s) of the identified languages added to the source files.");
    }
    if rules.rules.is_empty() {
        return Ok(None);
    }
    let role_map = rules.classify(&ingest.files);
    println!("Stack: {}", rules.stack);
    Ok(Some((rules.stack, role_map)))
}

/// Identifies the file roles, then reads the entities and the entry points
/// from the files of those roles: the application surface the domains are
/// clustered from. Each pass is incremental. If no role rules can be
/// identified the surface is empty and the domains fall back to the
/// directory summaries alone.
async fn build_surface(
    repo_root: &Path,
    ingest: &mut IngestResult,
    brief: &ProductBrief,
    llm: &PassProviders,
    force: bool,
    config: &Config,
    tracker: &UsageTracker,
) -> anyhow::Result<(Surface, EntryPoints, Option<RoleMap>)> {
    let Some((stack, role_map)) =
        identify_file_roles(repo_root, ingest, brief, llm, force, tracker).await?
    else {
        tracing::warn!("no file role rules identified, domains are clustered without the surface");
        return Ok((Surface::default(), EntryPoints::default(), None));
    };

    let business = super::business_files::locate(
        repo_root,
        ingest,
        &stack,
        brief,
        llm.get("business-files")?.as_ref(),
        force,
        tracker,
    )
    .await?;
    let business_files = business.files(&ingest.files);

    // One reading of the business files for both passes below, which then read the rest.
    if config.pipeline.business_read {
        println!("Reading the business files once for the entities and the entry points…");
        tracker
            .in_pass(
                "business-read",
                retrodoc_pipeline::read_business_files(
                    repo_root,
                    &role_map,
                    &business_files,
                    brief,
                    llm.get("glossary")?.as_ref(),
                    llm.get("entry-points")?.as_ref(),
                ),
            )
            .await
            .context("failed to read the business files")?;
    }

    println!("Reading the business entities…");
    let glossary = tracker
        .in_pass(
            "glossary",
            retrodoc_pipeline::build_glossary(
                repo_root,
                &role_map,
                &business_files,
                brief,
                llm.get("glossary")?.as_ref(),
                retrodoc_pipeline::Retention::OnlyRead,
            ),
        )
        .await
        .context("failed to build the glossary")?;
    println!("Reading the entry points…");
    let entry_points = tracker
        .in_pass(
            "entry-points",
            retrodoc_pipeline::build_entry_points(
                repo_root,
                &role_map,
                &business_files,
                brief,
                llm.get("entry-points")?.as_ref(),
                retrodoc_pipeline::Retention::OnlyRead,
            ),
        )
        .await
        .context("failed to build the entry points inventory")?;
    let entry_points = application_entry_points(entry_points, repo_root, source_paths(ingest));

    let surface = Surface::new(&glossary, &entry_points);
    if let Some(notice) = surface.missing_entities_notice(source_paths(ingest).count()) {
        tracing::info!("{notice}");
    }
    println!(
        "Surface: {} entit(ies), {} resource(s) with entry points.",
        surface.entities.len(),
        surface.resources.len()
    );
    Ok((surface, entry_points, Some(role_map)))
}

/// The entry points the later passes work from: in an application, without the public methods
/// that no route, command, job, consumer or webhook reaches (the saved inventory keeps them).
fn application_entry_points<'a>(
    entry_points: EntryPoints,
    repo_root: &Path,
    sources: impl IntoIterator<Item = &'a Path>,
) -> EntryPoints {
    let (kept, left_out) = entry_points.without_unreached_public_api(repo_root, sources);
    if !left_out.is_empty() {
        tracing::info!(
            count = left_out.len(),
            entry_points = ?left_out,
            "public API left out of the entry points: no route, job or webhook reaches it"
        );
    }
    kept
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

/// Removes the cached results of the LLM passes (see
/// [`Artifact::cleared_by_force`] for what is kept and why).
fn clear_caches(repo_root: &Path) -> anyhow::Result<()> {
    for artifact in Artifact::ALL {
        if !artifact.cleared_by_force() {
            continue;
        }
        let file = artifact.path(repo_root);
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

async fn run_confidence(
    repo_root: &Path,
    features: &mut [Feature],
    use_cases: &mut [UseCase],
    llm: &PassProviders,
    confidence: Confidence,
    concurrency: usize,
) -> anyhow::Result<()> {
    match confidence {
        Confidence::Sample(sample) => {
            println!("\nCross-checking use cases against the code (confidence)…");
            retrodoc_pipeline::score_confidence(
                repo_root,
                features,
                use_cases,
                llm.get("confidence")?.as_ref(),
                sample,
                concurrency,
            )
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
