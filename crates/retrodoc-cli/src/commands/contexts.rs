use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{Artifact, ContextMap, FileRole, RoleRules};

use super::workspace::{source_paths, Workspace};

/// Support files listed after the contexts.
const SUPPORT_SAMPLE: usize = 10;

/// Maps the bounded contexts (one LLM call over the mechanical groups, or the saved
/// `.retrodoc/cache/contexts.yaml`) and prints each with the files the current tree gives it.
/// `force` asks again, edits included. Needs the file roles (`retrodoc roles`) for the entry point
/// files the groups are built around.
pub async fn run(path: &Path, force: bool, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let mut ingest = workspace.ingest()?;
    ingest.existing_docs = super::docs::without_generated(ingest.existing_docs, config);
    let rules =
        RoleRules::load(repo_root).context("no file roles saved — run `retrodoc roles` first")?;
    rules.promote_sources(&mut ingest.files);
    let role_map = rules.classify(&ingest.files);
    let sources: Vec<_> = source_paths(&ingest).map(Path::to_path_buf).collect();
    let known: BTreeSet<&Path> = sources.iter().map(PathBuf::as_path).collect();
    let seeds: Vec<PathBuf> = role_map
        .files_with(FileRole::EntryPoint)
        .into_iter()
        .filter(|f| known.contains(f))
        .map(Path::to_path_buf)
        .collect();

    let llm = super::usage::pass_provider(&config.llm, tracker, "contexts")?;
    println!("Mapping the bounded contexts…");
    let map = tracker
        .in_pass(
            "contexts",
            retrodoc_pipeline::build_context_map(
                repo_root,
                &sources,
                &seeds,
                &super::brief::saved(repo_root),
                llm.as_ref(),
                force,
            ),
        )
        .await
        .context("failed to map the contexts")?;
    if map.is_empty() {
        anyhow::bail!(
            "no context mapped — see the warnings above (no entry point file, an unusable answer, \
             or an unreadable {}: fix it or use --force)",
            Artifact::Contexts.relative_path()
        );
    }

    let resolved = map.resolve(&sources, &seeds);
    println!("\nContexts, as saved (editable):");
    for context in &resolved.contexts {
        println!(
            "  {}  {} file(s), {} entry point file(s)  {}",
            context.slug,
            context.files.len(),
            context.entry_files.len(),
            context.name
        );
    }
    println!(
        "\n{} file(s) in no context (support):",
        resolved.support.len()
    );
    for file in resolved.support.iter().take(SUPPORT_SAMPLE) {
        println!("  {}", file.display());
    }
    if resolved.support.len() > SUPPORT_SAMPLE {
        println!("  … and {} more", resolved.support.len() - SUPPORT_SAMPLE);
    }
    if ContextMap::load(repo_root).as_ref() == Some(&map) {
        println!(
            "\n{} context(s) saved to {}.",
            map.contexts.len(),
            Artifact::Contexts.relative_path()
        );
    } else {
        println!(
            "\n{} context(s) NOT saved: the answer was unusable, so each group stands for a \
             context here and {} is left as it was; the next run without --force reuses it.",
            map.contexts.len(),
            Artifact::Contexts.relative_path()
        );
    }
    Ok(())
}
