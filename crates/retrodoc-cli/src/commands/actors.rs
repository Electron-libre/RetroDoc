use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_core::model::ActorKind;
use retrodoc_ingest::FileKind;
use retrodoc_llm::{HeartbeatProvider, OpenRouterProvider};
use retrodoc_pipeline::{EntryPoints, Glossary, RoleRules, Surface};

/// Identifies the business actors (who uses the application, in business
/// terms) from the authorization code and the user-like entities, saves
/// `.retrodoc/cache/actors.yaml` and prints them. The glossary and entry
/// points from the earlier commands are used when present. `force` ignores
/// the saved list.
pub async fn run(path: &Path, force: bool) -> anyhow::Result<()> {
    let repo_root = path
        .canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))?;
    let config = Config::load(&repo_root)
        .with_context(|| "config not found — run `retrodoc init` first".to_string())?;
    let mut ingest = retrodoc_ingest::run(&repo_root, &config.ingest)
        .with_context(|| format!("ingestion of {} failed", repo_root.display()))?;
    if let Some(rules) = RoleRules::load(&repo_root) {
        rules.promote_sources(&mut ingest.files);
    }
    let source_files: Vec<_> = ingest
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path.clone())
        .collect();
    let surface = Surface::new(
        &Glossary::load(&repo_root).unwrap_or_default(),
        &EntryPoints::load(&repo_root).unwrap_or_default(),
    );

    let llm = HeartbeatProvider::new(
        OpenRouterProvider::from_config(&config.llm)
            .context("could not initialize the LLM provider (missing API key?)")?,
    );
    let candidates = retrodoc_pipeline::authorization_files(&source_files);
    println!(
        "Identifying the actors ({} authorization file(s), {} entit(ies) known)…",
        candidates.len(),
        surface.entities.len()
    );
    let actors = retrodoc_pipeline::build_actors(&repo_root, &source_files, &surface, &llm, force)
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
    println!("\nSaved to .retrodoc/cache/actors.yaml.");
    Ok(())
}
