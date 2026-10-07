use std::path::Path;

use anyhow::Context;
use retrodoc_core::model::ActorKind;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{Artifact, EntryPoints, Glossary, RoleRules, Surface};

use super::workspace::{source_paths, Workspace};

/// Identifies the business actors (who uses the application, in business
/// terms) from the authorization code and the entities, saves
/// `.retrodoc/cache/actors.yaml` and prints them. The glossary and entry
/// points from the earlier commands are used when present. `force` ignores
/// the saved list.
pub async fn run(path: &Path, force: bool, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let mut ingest = workspace.ingest()?;
    if let Some(rules) = RoleRules::load(repo_root) {
        rules.promote_sources(&mut ingest.files);
    }
    let source_files: Vec<_> = source_paths(&ingest).map(Path::to_path_buf).collect();
    let surface = Surface::new(
        &Glossary::load(repo_root).unwrap_or_default(),
        &EntryPoints::load(repo_root).unwrap_or_default(),
    );

    let llm = super::usage::provider(&config.llm, tracker)?;
    tracker.set_pass("actors");
    let candidates = retrodoc_pipeline::authorization_files(&source_files);
    println!(
        "Identifying the actors ({} authorization file(s), {} entit(ies) known)…",
        candidates.len(),
        surface.entities.len()
    );
    let actors = retrodoc_pipeline::build_actors(repo_root, &source_files, &surface, &llm, force)
        .await
        .context("failed to identify the actors")?;

    println!("\nActors ({}):", actors.actors.len());
    for actor in &actors.actors {
        let kind = if actor.kind == ActorKind::Human {
            "human"
        } else {
            "system"
        };
        println!("  {} ({kind}): {}", actor.name, actor.description);
        if !actor.evidence.is_empty() {
            println!("    from: {}", actor.evidence.join(", "));
        }
    }
    println!("\nSaved to {}.", Artifact::Actors.relative_path());
    Ok(())
}
