//! The estimate `generate` prints before its first LLM call (`specs/run-estimate.md`): what the run knows
//! then (the source files within the budget, those changed since the last complete run, the saved artifacts
//! of the fixed passes, the passes it turns off), given to `retrodoc_pipeline::estimate`.

use std::path::{Path, PathBuf};

use retrodoc_core::config::{Config, DomainsFrom};
use retrodoc_ingest::{FileKind, IngestResult};
use retrodoc_pipeline::estimate::{estimate, EstimateInputs, RatioTable, RunEstimate};
use retrodoc_pipeline::{Artifact, RoleRules, SourceHashes};

use super::super::passes;
use super::{passes_of, Confidence, GenerateOptions};

/// The fixed passes and the artifact a rerun reuses them from. `domains` is left out: it is redone
/// whenever a file changed.
const FIXED_ARTIFACTS: &[(&str, Artifact)] = &[
    (passes::SOURCES, Artifact::SignalSources),
    (passes::BRIEF, Artifact::Product),
    (passes::ROLES, Artifact::Roles),
    (passes::BUSINESS_FILES, Artifact::BusinessFiles),
    (passes::ACTORS, Artifact::Actors),
];

/// Prints the estimate of the run `options` asks on the walked repository, and returns it to be saved with
/// the run.
pub(super) fn print(
    repo_root: &Path,
    config: &Config,
    options: &GenerateOptions,
    ingest: &IngestResult,
) -> RunEstimate {
    let contexts = config.pipeline.domains_from == DomainsFrom::Contexts;
    let passes = passes_of(config.pipeline.domains_from);
    let sources = source_files(repo_root, ingest);
    let budget = options.max_files.or(config.ingest.max_files);
    // On the surface path the passes still read every entry point whatever the budget; with the contexts,
    // the budget is what one run reads, a slice of its own.
    let source_files = if contexts {
        sources
            .len()
            .min(budget.unwrap_or(super::contexts::DEFAULT_RUN_FILES))
    } else {
        sources.len()
    };
    let changed_files = if options.force {
        None
    } else {
        changed_since_last_run(repo_root, &sources, if contexts { None } else { budget })
            .map(|n| n.min(source_files))
    };
    let saved_fixed: Vec<&str> = FIXED_ARTIFACTS
        .iter()
        .filter(|(_, artifact)| !(options.force && artifact.cleared_by_force()))
        .filter(|(pass, _)| passes.contains(pass))
        .filter(|(_, artifact)| artifact.path(repo_root).is_file())
        .map(|(pass, _)| *pass)
        .collect();
    let mut off = Vec::new();
    if matches!(options.confidence, Confidence::Skip) {
        off.push(passes::CONFIDENCE);
    }
    if !config.pipeline.rules {
        off.push(passes::RULES);
    }
    if !config.pipeline.reconcile || ingest.existing_docs.is_empty() {
        off.push(passes::RECONCILE);
    }
    // Without the joint reading, the glossary and the entry points read their files apart: not measured. It
    // needs one model for both passes, else each reads alone.
    let unmeasured: &[&str] = if joint_reading(config) {
        &[]
    } else {
        off.push(passes::BUSINESS_READ);
        &[passes::GLOSSARY, passes::ENTRY_POINTS]
    };
    let confidence_sample = match options.confidence {
        Confidence::Sample(sample) => sample,
        Confidence::Skip => None,
    };
    let inputs = EstimateInputs {
        passes: &passes,
        source_files,
        changed_files,
        saved_fixed: &saved_fixed,
        off: &off,
        unmeasured,
        confidence_sample,
    };
    let table = RatioTable::embedded();
    let models = pass_models(config, &passes);
    let estimate = estimate(&inputs, table);
    print!("{}", estimate.render(&models, table));
    estimate
}

/// Whether the run reads the business files once for the glossary and the entry points: on, and one model
/// for both passes (`retrodoc_pipeline::read_business_files`).
fn joint_reading(config: &Config) -> bool {
    config.pipeline.business_read
        && config.llm.for_pass(passes::GLOSSARY).model
            == config.llm.for_pass(passes::ENTRY_POINTS).model
}

/// The model each pass asks; `business-read` asks that of the glossary (the same as the entry points' when
/// it runs).
fn pass_models<'a>(config: &Config, names: &[&'a str]) -> Vec<(&'a str, String)> {
    names
        .iter()
        .map(|&pass| {
            let provider = if pass == passes::BUSINESS_READ {
                passes::GLOSSARY
            } else {
                pass
            };
            (pass, config.llm.for_pass(provider).model)
        })
        .collect()
}

/// The source files of the walk, with the languages a saved `roles.yaml` promotes, as the run will see them.
fn source_files(repo_root: &Path, ingest: &IngestResult) -> Vec<PathBuf> {
    let mut files = ingest.files.clone();
    if let Some(rules) = RoleRules::load(repo_root) {
        super::super::roles::promote_sources(&rules, &mut files);
    }
    files
        .into_iter()
        .filter(|file| file.kind == FileKind::Source)
        .map(|file| file.path)
        .collect()
}

/// The source files changed, removed or added since the last complete `generate`; `None` when there was
/// none. A file added counts within the room `budget` leaves besides the saved files still there.
fn changed_since_last_run(
    repo_root: &Path,
    sources: &[PathBuf],
    budget: Option<usize>,
) -> Option<usize> {
    if !Artifact::SourceHashes.path(repo_root).is_file() {
        return None;
    }
    let saved = SourceHashes::load(repo_root);
    let saved_paths: Vec<PathBuf> = saved.paths().map(PathBuf::from).collect();
    // The saved files hashed again (a missing one is left out): whether a file is a source may have changed
    // since, through the extensions the roles promote.
    let now = SourceHashes::capture_paths(repo_root, &saved_paths);
    let changed = saved
        .paths()
        .filter(|path| now.get(path) != saved.get(path))
        .count();
    let removed = saved.paths().filter(|path| now.get(path).is_none()).count();
    let added = sources
        .iter()
        .filter(|path| saved.get(&path.to_string_lossy()).is_none())
        .count();
    let room = budget.map_or(added, |budget| {
        budget
            .saturating_sub(saved_paths.len() - removed)
            .min(added)
    });
    Some(changed + room)
}
