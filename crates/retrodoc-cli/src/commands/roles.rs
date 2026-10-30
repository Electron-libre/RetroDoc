use std::path::Path;

use anyhow::Context;
use retrodoc_ingest::{FileEntry, FileKind};
use retrodoc_llm::UsageTracker;
use retrodoc_pipeline::{Artifact, FileRole, RoleRules};

use super::workspace::Workspace;

/// Unclassified files listed after the distribution.
const UNCLASSIFIED_SAMPLE: usize = 20;

/// Marks as source (or test) the files of the languages `rules` names, and says how many (nothing
/// when none): what every command shows of the sources, `generate` reads.
pub fn promote_sources(rules: &RoleRules, files: &mut [FileEntry]) {
    let promoted = rules.promote_sources(files);
    if promoted > 0 {
        println!(
            "{promoted} file(s) of the languages of {} counted as source or test.",
            Artifact::Roles.file_name()
        );
    }
}

/// Identifies the stack and the file role rules (one LLM call, or the saved
/// `.retrodoc/cache/roles.yaml`), gives the files no rule matches theirs (a
/// second call, once per set of such files), applies them and prints the
/// distribution.
/// `force` ignores the saved rules and asks the LLM again.
pub async fn run(path: &Path, force: bool, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let mut ingest = workspace.ingest()?;
    ingest.existing_docs = super::docs::without_generated(ingest.existing_docs, config);

    let reused = (!force).then(|| RoleRules::load(repo_root)).flatten();
    let saved = reused.is_some();
    // The provider is opened only when a call is due, so reused rules need no API key.
    let rules = if let Some(rules) = reused.filter(|r| r.unmatched_to_ask(&ingest.files).is_empty())
    {
        rules
    } else {
        let llm = super::usage::pass_provider(&config.llm, tracker, "roles")?;
        tracker.set_pass("roles");
        retrodoc_pipeline::identify_roles(
            repo_root,
            &ingest,
            &super::brief::saved(repo_root),
            llm.as_ref(),
            force,
        )
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
    if !rules.unmatched.rules.is_empty() {
        println!(
            "\nRules for the files no rule above matches ({}, applied to those files only):",
            Artifact::RolesUnmatched.relative_path()
        );
        for rule in &rules.unmatched.rules {
            println!("  {:<12} {}", rule.role.label(), rule.pattern);
        }
    }

    if rules.chunk_boundaries.is_empty() {
        println!(
            "\nChunk boundaries: none (languages parsed with tree-sitter are cut at their definitions, \
             the others at blank lines; `--force` to identify them)"
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

    println!();
    promote_sources(&rules, &mut ingest.files);
    let count = |kind: FileKind| ingest.files.iter().filter(|f| f.kind == kind).count();
    println!(
        "Files: {} (source: {}, test: {})",
        ingest.files.len(),
        count(FileKind::Source),
        count(FileKind::Test)
    );

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
