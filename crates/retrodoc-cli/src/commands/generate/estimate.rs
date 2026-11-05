//! The estimate `generate` prints before its first LLM call (`specs/run-estimate.md`): what the run knows
//! then (the source files within the budget, those changed since the last complete run, the saved artifacts
//! of the fixed passes, the passes it turns off), given to `retrodoc_pipeline::estimate`.

use std::path::{Path, PathBuf};

use retrodoc_core::config::{Config, DomainsFrom};
use retrodoc_ingest::IngestResult;
use retrodoc_pipeline::estimate::{estimate, EstimateInputs, RatioTable, Shown};
use retrodoc_pipeline::{Artifact, SourceHashes};

use super::super::workspace::source_paths;
use super::{Confidence, GenerateOptions, GENERATE_PASSES};

/// The fixed passes and the artifact a rerun reuses them from. `domains` is left out: it is redone
/// whenever a file changed.
const FIXED_ARTIFACTS: &[(&str, Artifact)] = &[
    ("sources", Artifact::SignalSources),
    ("brief", Artifact::Product),
    ("roles", Artifact::Roles),
    ("business-files", Artifact::BusinessFiles),
    ("actors", Artifact::Actors),
];

/// Prints the estimate of the run `options` asks on the walked repository.
pub(super) fn print(
    repo_root: &Path,
    config: &Config,
    options: &GenerateOptions,
    ingest: &IngestResult,
) {
    let contexts = config.pipeline.domains_from == DomainsFrom::Contexts;
    let passes: Vec<&str> = GENERATE_PASSES
        .iter()
        .copied()
        .filter(|&pass| contexts || pass != "contexts")
        .collect();
    let walked = source_paths(ingest).count();
    let budget = options.max_files.or(config.ingest.max_files);
    let budget = if contexts {
        Some(budget.unwrap_or(super::contexts::DEFAULT_RUN_FILES))
    } else {
        budget
    };
    let source_files = budget.map_or(walked, |budget| walked.min(budget));
    let changed_files = if options.force {
        None
    } else {
        // With the contexts, each run reads a slice of its own: the budget is not shared with the saved files.
        let shared_budget = if contexts { None } else { budget };
        changed_since_last_run(repo_root, ingest, shared_budget).map(|n| n.min(source_files))
    };
    let saved_fixed: Vec<&str> = FIXED_ARTIFACTS
        .iter()
        .filter(|(_, artifact)| !(options.force && artifact.cleared_by_force()))
        .filter(|(_, artifact)| artifact.path(repo_root).is_file())
        .map(|(pass, _)| *pass)
        .collect();
    let mut off = Vec::new();
    if matches!(options.confidence, Confidence::Skip) {
        off.push("confidence");
    }
    if !config.pipeline.rules {
        off.push("rules");
    }
    if !config.pipeline.reconcile || ingest.existing_docs.is_empty() {
        off.push("reconcile");
    }
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
        confidence_sample,
    };
    let table = RatioTable::embedded();
    let models: Vec<(&str, String)> = passes
        .iter()
        .map(|&pass| (pass, config.llm.for_pass(pass).model))
        .collect();
    let shown = Shown {
        source_files,
        changed_files,
        models: &models,
    };
    print!("{}", estimate(&inputs, table).render(&shown, table));
}

/// The source files changed, removed or added since the last complete `generate`; `None` when there was
/// none. A file added counts within the room `budget` leaves besides the saved files.
fn changed_since_last_run(
    repo_root: &Path,
    ingest: &IngestResult,
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
    let added = source_paths(ingest)
        .filter(|path| saved.get(&path.to_string_lossy()).is_none())
        .count();
    let room = budget.map_or(added, |budget| {
        budget.saturating_sub(saved_paths.len()).min(added)
    });
    Some(changed + room)
}
