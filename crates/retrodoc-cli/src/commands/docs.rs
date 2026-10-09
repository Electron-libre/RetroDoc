use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_pipeline::DomainMap;
use retrodoc_render::RunMetadata;

/// Renders the docs from the artifacts in `.retrodoc/cache/` and writes them
/// under the configured docs dir. With `dry_run`, only previews what would
/// change (files and diffs) and writes nothing. Files whose content already
/// matches are left untouched, so a rerun on unchanged artifacts is a no-op.
pub fn publish(repo_root: &Path, config: &Config, dry_run: bool) -> anyhow::Result<()> {
    let (Some(features), Some(use_cases)) = (
        retrodoc_pipeline::load_features(repo_root),
        retrodoc_pipeline::load_use_cases(repo_root),
    ) else {
        anyhow::bail!("no features/use cases found — run `retrodoc generate` first");
    };
    let domains = DomainMap::load(repo_root).unwrap_or_default();

    let mut report = retrodoc_pipeline::build_report(&domains, &features, &use_cases);
    report.skipped_files = retrodoc_pipeline::Scope::load(repo_root)
        .map(|s| s.skipped)
        .unwrap_or_default();
    let metadata = RunMetadata {
        model: config.llm.model.clone(),
        commit: retrodoc_ingest::git_history::head_commit(repo_root),
        generated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    let files = retrodoc_render::render(
        &retrodoc_pipeline::domain_models(&domains, &features),
        &features,
        &use_cases,
        &report.to_markdown(),
        &metadata,
    )?;

    let docs_dir = repo_root.join(&config.output.docs_dir);
    let plan = retrodoc_render::plan(&docs_dir, files)
        .with_context(|| format!("could not compare with {}", docs_dir.display()))?;

    println!();
    print!("{}", plan.preview());
    if dry_run {
        println!(
            "\nDry run: {} file(s) would be written to {}, nothing was.",
            plan.change_count(),
            config.output.docs_dir
        );
        return Ok(());
    }
    let written = plan
        .apply(&docs_dir)
        .with_context(|| format!("could not write to {}", docs_dir.display()))?;
    println!("\n{written} file(s) written to {}.", config.output.docs_dir);
    Ok(())
}

/// Drops from the input docs what `RetroDoc` itself generated into the docs
/// dir, so that a rerun doesn't cluster its own output.
pub fn without_generated(
    docs: Vec<retrodoc_ingest::ExistingDoc>,
    config: &Config,
) -> Vec<retrodoc_ingest::ExistingDoc> {
    let generated = super::workspace::generated_dirs(config);
    docs.into_iter()
        .filter(|d| !generated.iter().any(|g| d.path.starts_with(g)))
        .collect()
}

/// The generated docs the `search` and `mcp` commands answer from: the
/// artifacts of the last `generate` run and the project's own Markdown docs.
pub fn load_docs(workspace: &super::workspace::Workspace) -> anyhow::Result<retrodoc_mcp::Docs> {
    let docs = retrodoc_ingest::existing_docs::load_existing_docs(
        &workspace.repo_root,
        &workspace.config.ingest.existing_docs_paths,
    )
    .context("could not read the existing docs")?;
    let docs = without_generated(docs, &workspace.config);
    retrodoc_mcp::Docs::load(&workspace.repo_root, &docs)
        .context("no features/use cases found — run `retrodoc generate` first")
}
