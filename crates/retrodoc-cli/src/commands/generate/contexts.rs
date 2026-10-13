//! `generate` with `[pipeline] domains_from = "contexts"` (ADR 0029, step 4 of
//! `issues/done/business_files_cap_on_large_application.md`): one run documents a slice of the
//! application, some bounded contexts, and builds on the earlier runs without redoing them.
//!
//! Roles, then the map of the contexts, then the slice of this run (`--context`, else the next
//! contexts not done within `--max-files`); the files of the slice are read (the readers keep the
//! inventories of the other contexts), a context is a domain, the features of the other contexts
//! stay as they were, and the use cases, rules and confidence run on every feature (the earlier
//! ones are found up to date and cost nothing). The contexts of the slice are then marked done.

use std::path::{Path, PathBuf};

use anyhow::Context;
use retrodoc_ingest::IngestResult;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{
    ContextMap, DomainMap, EntryPoints, FileRole, Glossary, Retention, RoleMap, RunSlice,
    SourceHashes, Surface,
};

use super::super::usage::PassProviders;
use super::super::workspace::{source_paths, Workspace};
use super::{GenerateOptions, Passes, Signals, VOCABULARY_SIZE};

/// Files one run reads when `--max-files` says nothing: the size above which a context is split.
const DEFAULT_RUN_FILES: usize = 150;

/// The repository, as the passes of a run see it.
struct Repo<'a> {
    ingest: &'a IngestResult,
    sources: &'a [PathBuf],
    seeds: &'a [PathBuf],
    role_map: &'a RoleMap,
    map: &'a ContextMap,
    slice: &'a RunSlice,
}

pub(super) async fn run(
    workspace: &Workspace,
    llm: &PassProviders,
    tracker: &UsageTracker,
    options: &GenerateOptions,
    signals: Signals,
    mut ingest: IngestResult,
) -> anyhow::Result<()> {
    let (repo_root, config) = (workspace.repo_root.as_path(), &workspace.config);
    let Some((_, role_map)) = super::identify_file_roles(
        repo_root,
        &mut ingest,
        &signals.brief,
        llm,
        options.force,
        config,
        tracker,
    )
    .await?
    else {
        anyhow::bail!("no file role could be identified, and the contexts are built from them");
    };
    let sources: Vec<PathBuf> = source_paths(&ingest).map(Path::to_path_buf).collect();
    let seeds = entry_point_files(&role_map, &sources);

    // Not forced: the map is hand-editable, `retrodoc contexts --force` asks again.
    let map = tracker
        .in_pass("contexts", async {
            retrodoc_pipeline::build_context_map(
                repo_root,
                &sources,
                &seeds,
                &signals.brief,
                llm.get("contexts")?.as_ref(),
                false,
            )
            .await
            .context("failed to map the contexts")
        })
        .await?;
    anyhow::ensure!(
        !map.is_empty(),
        "no context mapped: the application has no entry point file, or the map is unusable"
    );
    let budget = options
        .max_files
        .or(config.ingest.max_files)
        .unwrap_or(DEFAULT_RUN_FILES);
    let slice = retrodoc_pipeline::plan_run(
        repo_root,
        &map,
        &sources,
        &seeds,
        &role_map,
        &options.contexts,
        budget,
    )?;
    print_slice(&slice);

    if !slice.contexts.is_empty() {
        let repo = Repo {
            ingest: &ingest,
            sources: &sources,
            seeds: &seeds,
            role_map: &role_map,
            map: &map,
            slice: &slice,
        };
        let passes = Passes {
            repo_root,
            config,
            llm,
            tracker,
        };
        document(&passes, options, signals, &repo).await?;
        print_pending(&slice);
    }
    super::super::docs::publish(repo_root, config, options.dry_run)
}

/// The entry point files of the sources.
fn entry_point_files(role_map: &RoleMap, sources: &[PathBuf]) -> Vec<PathBuf> {
    let known: std::collections::BTreeSet<&Path> = sources.iter().map(PathBuf::as_path).collect();
    role_map
        .files_with(FileRole::EntryPoint)
        .into_iter()
        .filter(|f| known.contains(f))
        .map(Path::to_path_buf)
        .collect()
}

/// Reads the slice, derives everything from it and marks its contexts done.
async fn document(
    passes: &Passes<'_>,
    options: &GenerateOptions,
    signals: Signals,
    repo: &Repo<'_>,
) -> anyhow::Result<()> {
    let Passes {
        repo_root,
        config,
        llm,
        tracker,
    } = *passes;
    let (glossary, entry_points) = read_slice(passes, &signals, repo).await?;
    let surface = Surface::new(&glossary, &entry_points);
    let actors = tracker
        .in_pass(
            "actors",
            super::identify_actors(
                repo_root,
                repo.ingest,
                &surface,
                &signals.brief,
                llm,
                options.force,
                true,
            ),
        )
        .await?;
    let slice = repo.slice;
    // Read before the passes: a file edited during the run then reads as modified later.
    let source_hashes = SourceHashes::capture_paths(repo_root, &documented_files(repo));
    let (in_run, everywhere) = domains(repo, &glossary, &entry_points);
    everywhere
        .save(repo_root)
        .context("could not save the domains")?;
    super::print::surface_domains(&in_run);

    println!("\nDeriving features from the entry points…");
    let mut features = tracker
        .in_pass(
            "features",
            retrodoc_pipeline::build_features_from_surface(
                repo_root,
                &in_run,
                &entry_points,
                &surface,
                &signals.brief,
                &signals.evidence,
                llm.get("features")?.as_ref(),
                Retention::Others,
            ),
        )
        .await
        .context("failed to derive the features from the entry points")?;
    repo.map.arrange(&mut features);
    retrodoc_pipeline::save_features(repo_root, &features)
        .context("could not save the features")?;
    report_entries_by_context(repo_root, repo, &in_run)?;

    let mut context =
        super::use_case_context(entry_points, repo.ingest, actors, &surface, signals, config);
    context.domain_vocabulary =
        retrodoc_pipeline::domain_vocabulary(&everywhere, &glossary, VOCABULARY_SIZE);
    let mut use_cases = tracker
        .in_pass(
            "use-cases",
            super::derive_use_cases(
                repo_root,
                &features,
                &context,
                &surface,
                llm,
                super::Scoring::NewOnly,
            ),
        )
        .await?;
    super::finish_chain(
        passes,
        &mut features,
        &mut use_cases,
        context,
        options.confidence,
    )
    .await?;

    let mut recorded = SourceHashes::load(repo_root);
    recorded.merge(source_hashes);
    recorded
        .save(repo_root)
        .context("could not save the source hashes")?;
    for slug in &slice.contexts {
        retrodoc_pipeline::mark_done(
            repo_root,
            repo.map,
            repo.sources,
            repo.seeds,
            repo.role_map,
            slug,
        )?;
    }
    super::print::features(&features, &use_cases);
    super::print::saved(features.len(), use_cases.len());
    Ok(())
}

/// The files the docs of this run describe: those of the contexts of the slice and the ones read
/// for them. The docs say nothing of the code of the contexts not documented.
fn documented_files(repo: &Repo<'_>) -> Vec<PathBuf> {
    let resolved = repo.map.resolve(repo.sources, repo.seeds);
    let slice = repo.slice;
    resolved
        .contexts
        .iter()
        .filter(|c| slice.contexts.contains(&c.slug))
        .flat_map(|c| {
            c.files
                .iter()
                .chain(slice.reads.get(&c.slug).into_iter().flatten())
                .cloned()
        })
        .collect()
}

/// Drops from the saved coverage the rows of a context cut again or removed since, then prints
/// those of the contexts of the run.
fn report_entries_by_context(
    repo_root: &Path,
    repo: &Repo<'_>,
    in_run: &DomainMap,
) -> anyhow::Result<()> {
    let Some(mut coverage) = retrodoc_pipeline::BehaviourCoverage::load(repo_root) else {
        return Ok(());
    };
    let before = coverage.by_domain.len();
    coverage
        .by_domain
        .retain(|slug, _| repo.map.contexts.iter().any(|c| c.slug == *slug));
    if coverage.by_domain.len() != before {
        coverage
            .save(repo_root)
            .context("could not save the behaviour coverage")?;
    }
    super::print::entries_by_context(&coverage, in_run);
    Ok(())
}

/// The domains of the contexts of this run, and those of every context documented so far (the
/// ones the docs gather).
fn domains(
    repo: &Repo<'_>,
    glossary: &Glossary,
    entry_points: &EntryPoints,
) -> (DomainMap, DomainMap) {
    let resolved = repo.map.resolve(repo.sources, repo.seeds);
    let slice = repo.slice;
    let documented: Vec<String> = repo
        .map
        .contexts
        .iter()
        .map(|c| c.slug.clone())
        .filter(|slug| slice.contexts.contains(slug) || slice.done.contains(slug))
        .collect();
    let of = |slugs: &[String]| {
        retrodoc_pipeline::domains_from_contexts(
            &resolved,
            &slice.reads,
            glossary,
            entry_points,
            slugs,
        )
    };
    (of(&slice.contexts), of(&documented))
}

/// Reads the files of the slice for the entities and the entry points, keeping the inventories of
/// the contexts read before.
async fn read_slice(
    passes: &Passes<'_>,
    signals: &Signals,
    repo: &Repo<'_>,
) -> anyhow::Result<(Glossary, EntryPoints)> {
    let Passes {
        repo_root,
        config,
        llm,
        tracker,
    } = *passes;
    let brief = &signals.brief;
    let files = &repo.slice.files;
    // The roles of the slice only: the passes read the entry point files of the roles they are given.
    let roles = repo.role_map.only(files);
    println!("\nReading {} file(s) of the slice…", files.len());
    if config.pipeline.business_read {
        tracker
            .in_pass(
                "business-read",
                retrodoc_pipeline::read_business_files(
                    repo_root,
                    &roles,
                    files,
                    brief,
                    llm.get("glossary")?.as_ref(),
                    llm.get("entry-points")?.as_ref(),
                ),
            )
            .await
            .context("failed to read the files of the slice")?;
    }
    let glossary = tracker
        .in_pass(
            "glossary",
            retrodoc_pipeline::build_glossary(
                repo_root,
                &roles,
                files,
                brief,
                llm.get("glossary")?.as_ref(),
                Retention::Others,
            ),
        )
        .await
        .context("failed to build the glossary")?;
    let entry_points = tracker
        .in_pass(
            "entry-points",
            retrodoc_pipeline::build_entry_points(
                repo_root,
                &roles,
                files,
                brief,
                llm.get("entry-points")?.as_ref(),
                Retention::Others,
            ),
        )
        .await
        .context("failed to build the entry points inventory")?;
    Ok((glossary, entry_points))
}

fn print_slice(slice: &RunSlice) {
    if slice.contexts.is_empty() {
        println!(
            "\nContexts: {} done, {} left{}.",
            slice.done.len(),
            slice.pending.len(),
            if slice.pending.is_empty() {
                ": every context is documented"
            } else {
                ", none selected (nothing fits --max-files?)"
            }
        );
    } else {
        println!(
            "\nContexts: {} done, {} to document now ({}), {} left.",
            slice.done.len(),
            slice.contexts.len(),
            slice.contexts.join(", "),
            slice.pending.len()
        );
    }
}

/// What a next run takes up.
fn print_pending(slice: &RunSlice) {
    if slice.pending.is_empty() {
        println!("\nAll the contexts are documented.");
    } else {
        println!(
            "\n{} context(s) left: {}. Run `retrodoc generate` again, or name them with --context.",
            slice.pending.len(),
            slice.pending.join(", ")
        );
    }
}
