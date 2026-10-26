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
    report.with_behaviour_coverage(retrodoc_pipeline::BehaviourCoverage::load(repo_root));
    let rules = retrodoc_pipeline::load_rules(repo_root).unwrap_or_default();
    if rules.is_empty() && retrodoc_pipeline::Artifact::Rules.path(repo_root).exists() {
        tracing::warn!(
            "{} could not be read or holds no rule: the docs are written without rules",
            retrodoc_pipeline::Artifact::Rules.relative_path()
        );
    }
    add_doc_debt(
        &mut report,
        repo_root,
        config,
        &domains,
        &features,
        &use_cases,
    );
    let metadata = RunMetadata {
        model: config.llm.model.clone(),
        commit: retrodoc_ingest::git_history::head_commit(repo_root),
        generated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    let files = retrodoc_render::render_with_rules(
        &retrodoc_pipeline::domain_models(&domains, &features),
        &features,
        &use_cases,
        &rules,
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

/// Adds to `report` where the generated items stand against the project's own docs, and what the
/// rules pass set aside. An unreadable doc leaves the reconciliation out (with a warning) rather than
/// listing every item as undocumented.
pub fn add_doc_debt(
    report: &mut retrodoc_pipeline::DebtReport,
    repo_root: &Path,
    config: &Config,
    domains: &DomainMap,
    features: &[retrodoc_core::model::Feature],
    use_cases: &[retrodoc_core::model::UseCase],
) {
    // Only while the rules are on: a file left by an earlier run is not what the last one judged.
    if config.pipeline.rules {
        report.with_set_aside(retrodoc_pipeline::load_set_aside(repo_root).unwrap_or_default());
    }
    let rules = retrodoc_pipeline::load_rules(repo_root).unwrap_or_default();
    match project_docs(repo_root, config) {
        Ok(docs) => report.with_reconciliation(retrodoc_pipeline::reconcile::reconcile(
            domains,
            features,
            use_cases,
            &rules,
            &docs,
            &retrodoc_pipeline::reconcile::Judgements::load(repo_root),
        )),
        Err(err) => tracing::warn!("items not compared with the project docs: {err:#}"),
    }
}

/// The project's own Markdown docs (what `generate` ingests, without what it wrote itself).
pub fn project_docs(
    repo_root: &Path,
    config: &Config,
) -> anyhow::Result<Vec<retrodoc_ingest::ExistingDoc>> {
    let docs = retrodoc_ingest::existing_docs::load_existing_docs(
        repo_root,
        &config.ingest.existing_docs_paths,
    )
    .context("could not read the existing docs")?;
    Ok(without_generated(docs, config))
}

/// The generated docs the `search` and `mcp` commands answer from: the
/// artifacts of the last `generate` run and the project's own Markdown docs.
pub fn load_docs(workspace: &super::workspace::Workspace) -> anyhow::Result<retrodoc_mcp::Docs> {
    let docs = project_docs(&workspace.repo_root, &workspace.config)?;
    retrodoc_mcp::Docs::load(&workspace.repo_root, &docs)
        .context("no features/use cases found — run `retrodoc generate` first")
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: REC-1
    #[test]
    fn the_generated_docs_are_not_matched_against_the_items() {
        let dir = tempfile::tempdir().unwrap();
        let write = |path: &str, content: &str| {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        };
        write("docs/guide.md", "# Riders\nRiders deliver the orders.\n");
        write(
            "docs/functional/billing/README.md",
            "# Payment\nPayment of the invoices.\n",
        );
        write("docs/_retrodoc/coverage-report.md", "# Payment\nPayment.\n");
        let feature = |slug: &str, name: &str| retrodoc_core::model::Feature {
            back_office: false,
            slug: slug.into(),
            domain_slug: "billing".into(),
            sub_domain_slug: None,
            name: name.into(),
            description: String::new(),
            source_paths: vec![],
            entry_points: Vec::new(),
            confidence: None,
        };
        let mut report = retrodoc_pipeline::DebtReport::default();

        add_doc_debt(
            &mut report,
            dir.path(),
            &Config::default(),
            &DomainMap::default(),
            &[feature("pay", "Payment"), feature("ride", "Riders")],
            &[],
        );

        let items = report.reconciliation.unwrap().items;
        let sections: Vec<(&str, Vec<String>)> = items
            .iter()
            .map(|i| {
                (
                    i.label.as_str(),
                    i.sections
                        .iter()
                        .map(retrodoc_pipeline::reconcile::SectionRef::origin)
                        .collect(),
                )
            })
            .collect();
        assert_eq!(
            sections,
            [
                ("billing/pay", vec![]),
                ("billing/ride", vec!["docs/guide.md#Riders".to_string()]),
            ]
        );
    }
}
