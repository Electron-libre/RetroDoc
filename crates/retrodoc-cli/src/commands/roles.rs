use std::path::Path;

use anyhow::Context;
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{Artifact, FileRole, RoleRules};

use super::workspace::Workspace;

/// Unclassified files listed after the distribution.
const UNCLASSIFIED_SAMPLE: usize = 20;

/// Identifies the stack and the file role rules (one LLM call, or the saved
/// `.retrodoc/cache/roles.yaml`), applies them and prints the distribution.
/// `force` ignores the saved rules and asks the LLM again.
pub async fn run(path: &Path, force: bool, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let mut ingest = workspace.ingest()?;
    ingest.existing_docs = super::docs::without_generated(ingest.existing_docs, config);

    let saved = !force && RoleRules::load(repo_root).is_some();
    let rules = if saved {
        RoleRules::load(repo_root).unwrap_or_default()
    } else {
        let llm = super::usage::provider(&config.llm, tracker)?;
        tracker.set_pass("roles");
        retrodoc_pipeline::identify_roles(repo_root, &ingest, &llm, force)
            .await
            .context("failed to identify the file roles")?
    };

    if rules.rules.is_empty() {
        anyhow::bail!(
            "no role rules identified (the LLM answer was unusable, see the warnings above) — try again"
        );
    }
    println!("Stack: {}", rules.stack);
    println!(
        "\nRules ({}, {} — editable):",
        if saved { "reused" } else { "identified" },
        Artifact::Roles.relative_path()
    );
    for rule in &rules.rules {
        println!("  {:<12} {}", rule.role.label(), rule.pattern);
    }

    if rules.chunk_boundaries.is_empty() {
        println!(
            "\nChunk boundaries: none (long files are cut at blank lines; `--force` to identify them)"
        );
    } else {
        println!("\nChunk boundaries (where a unit starts, used to cut long files):");
        for boundary in &rules.chunk_boundaries {
            println!(
                "  {:<12} {}",
                boundary.extensions.join(","),
                boundary.pattern
            );
        }
    }

    let map = rules.classify(&ingest.files);
    println!("\nDistribution ({} file(s)):", map.roles.len());
    for (role, count) in map.distribution() {
        println!("  {:<12} {count}", role.label());
    }

    let unclassified = map.files_with(FileRole::Unclassified);
    if !unclassified.is_empty() {
        println!("\nUnclassified, first {UNCLASSIFIED_SAMPLE}:");
        for file in unclassified.iter().take(UNCLASSIFIED_SAMPLE) {
            println!("  {}", file.display());
        }
    }
    Ok(())
}
