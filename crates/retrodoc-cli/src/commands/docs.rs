use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_render::RunMetadata;

/// Renders the docs from the artifacts in `.retrodoc/cache/`, the human edits of
/// `.retrodoc/edits.json` shown over them, and writes them under the configured docs dir, with the
/// debt report `report` prints, which lists the sections `.retrodoc/reviews.json` marks wrong. With
/// `dry_run`, only previews what would change (files and diffs) and writes nothing. Files whose
/// content already matches are left untouched, so a rerun on unchanged artifacts, edits and reviews
/// is a no-op.
pub fn publish(repo_root: &Path, config: &Config, dry_run: bool) -> anyhow::Result<()> {
    retrodoc_pipeline::cache_format::check(repo_root)?;
    if retrodoc_pipeline::load_use_cases(repo_root).is_none() {
        anyhow::bail!(NO_DOCS);
    }
    // Before anything is written: docs without the team's corrections would overwrite theirs.
    let edits = retrodoc_mcp::edits::load(repo_root)
        .context("the docs are not written: the human edits cannot be read")?;
    let docs =
        retrodoc_mcp::GeneratedDocs::load_with_edits(repo_root, &[], edits)?.context(NO_DOCS)?;
    let report = debt_report(&docs, repo_root, config);
    let rules = docs.rules();
    if rules.is_empty()
        && retrodoc_core::cache::Artifact::Rules
            .path(repo_root)
            .exists()
    {
        tracing::warn!(
            "{} could not be read or holds no rule: the docs are written without rules",
            retrodoc_core::cache::Artifact::Rules.relative_path()
        );
    }
    let metadata = RunMetadata {
        model: config.llm.model.clone(),
        commit: retrodoc_ingest::git_history::head_commit(repo_root),
        generated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    let links = report
        .reconciliation
        .as_ref()
        .map(|r| doc_links(r, rules, &config.output.docs_dir))
        .unwrap_or_default();
    let files = retrodoc_render::render_with_docs(
        docs.domains(),
        docs.features(),
        docs.use_cases(),
        rules,
        &links,
        &edit_marks(docs.edits()),
        retrodoc_render::Texts::for_tag(config.output.language_tag()),
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
            "\nDry run: {} file(s) would be written to {} and {} removed, nothing was.",
            plan.change_count(),
            config.output.docs_dir,
            plan.removed.len()
        );
        return Ok(());
    }
    let written = plan
        .apply(&docs_dir)
        .with_context(|| format!("could not write to {}", docs_dir.display()))?;
    println!(
        "\n{written} file(s) written to {} and {} removed.",
        config.output.docs_dir,
        plan.removed.len()
    );
    Ok(())
}

/// The mark of each edit that changes a field the readers show (SED-33, SED-38), by the item of
/// its page; an edit of a section the docs no longer hold marks no page.
fn edit_marks(edits: &retrodoc_mcp::edits::Edits) -> retrodoc_render::EditMarks {
    use retrodoc_mcp::edits::Field;
    use retrodoc_mcp::store::SectionKind;
    use retrodoc_render::{EditMark, EditedField, ItemRef};

    edits
        .edits()
        .iter()
        .filter_map(|edit| {
            let fields: Vec<EditedField> = edit
                .changed()
                .into_iter()
                .map(|field| match field {
                    Field::Name => EditedField::Name,
                    Field::Description => EditedField::Description,
                    Field::Narrative => EditedField::Narrative,
                })
                .collect();
            if fields.is_empty() {
                return None;
            }
            let id = edit.section.id.clone();
            let item = match edit.section.kind {
                SectionKind::Domain => ItemRef::Domain(id),
                SectionKind::Feature => ItemRef::Feature(id),
                SectionKind::UseCase => {
                    let (feature, use_case) = id.split_once('/')?;
                    ItemRef::UseCase {
                        feature: feature.to_string(),
                        use_case: use_case.to_string(),
                    }
                }
            };
            let mark = EditMark {
                fields,
                author: edit.author.clone(),
                edited_at: edit.edited_at.clone(),
            };
            Some((item, mark))
        })
        .collect()
}

/// Drops from the input docs what `RetroDoc` itself generated into the docs
/// dir, so that a rerun doesn't cluster its own output.
pub fn without_generated(
    docs: Vec<retrodoc_ingest::ExistingDoc>,
    config: &Config,
) -> Vec<retrodoc_ingest::ExistingDoc> {
    let generated = config.output.generated_dirs();
    docs.into_iter()
        .filter(|d| !generated.iter().any(|g| d.path.starts_with(g)))
        .collect()
}

/// The doc sections behind the items of the last run, from the reconciliation `generated` made
/// when it was loaded (no LLM call); none before a first `generate` or with no collected doc, the
/// docs dir set all the same (the web resolves the links of a narrative from it, WEB-123).
pub fn item_doc_links(
    generated: Option<&retrodoc_mcp::GeneratedDocs>,
    config: &Config,
) -> retrodoc_render::DocLinks {
    generated
        .and_then(|g| {
            let reconciliation = g.uncovered().reconciliation.as_ref()?;
            Some(doc_links(
                reconciliation,
                g.rules(),
                &config.output.docs_dir,
            ))
        })
        .unwrap_or_else(|| retrodoc_render::DocLinks {
            docs_dir: config.output.docs_dir.clone().into(),
            ..retrodoc_render::DocLinks::default()
        })
}

/// The doc sections each item page links: those that document or contradict the item. A rule that
/// cites a section already shows it with its evidence.
fn doc_links(
    reconciliation: &retrodoc_pipeline::reconcile::Reconciliation,
    rules: &[retrodoc_core::model::BusinessRule],
    docs_dir: &str,
) -> retrodoc_render::DocLinks {
    use retrodoc_pipeline::reconcile::{ItemKind, Status};
    use retrodoc_render::{DocLink, ItemRef};
    let mut links = retrodoc_render::DocLinks {
        docs_dir: docs_dir.into(),
        ..retrodoc_render::DocLinks::default()
    };
    for item in &reconciliation.items {
        if !matches!(item.status, Status::Documented | Status::Contradicted) {
            continue;
        }
        let mut parts = item.label.rsplit('/');
        let (last, before) = (parts.next().unwrap_or_default(), parts.next());
        let key = match item.kind {
            ItemKind::Domain => ItemRef::Domain(item.label.clone()),
            ItemKind::Feature => ItemRef::Feature(last.to_string()),
            ItemKind::UseCase => ItemRef::UseCase {
                feature: before.unwrap_or_default().to_string(),
                use_case: last.to_string(),
            },
            ItemKind::Rule => {
                let cites = rules.iter().any(|r| {
                    r.slug == last
                        && r.evidence
                            .iter()
                            .any(|e| matches!(e, retrodoc_core::model::RuleEvidence::Doc { .. }))
                });
                if cites {
                    continue;
                }
                ItemRef::Rule(last.to_string())
            }
        };
        let docs = item
            .sections
            .iter()
            .map(|section| DocLink {
                path: section.path.clone(),
                heading: section.heading.clone(),
                contradicts: item
                    .contradictions
                    .iter()
                    .find(|c| c.section == *section)
                    .map(|c| c.doc_states.clone()),
            })
            .collect();
        links.links.insert(key, docs);
    }
    links
}

/// The debt report of `docs`, loaded from `repo_root`, as `report` prints it and `render` writes it
/// (SED-39). Project docs that can't be read leave the comparison with them out, and reviews that
/// can't be read the sections marked wrong, each with a warning, rather than listing every item as
/// undocumented or none as reviewed.
pub fn debt_report(
    docs: &retrodoc_mcp::GeneratedDocs,
    repo_root: &Path,
    config: &Config,
) -> retrodoc_pipeline::DebtReport {
    let collected = project_docs(repo_root, config)
        .inspect_err(|err| tracing::warn!("items not compared with the project docs: {err:#}"))
        .ok();
    let reviews = retrodoc_mcp::reviews::load(repo_root)
        .inspect_err(|err| tracing::warn!("the report lists no section marked wrong: {err}"))
        .ok();
    docs.debt_report(&retrodoc_mcp::ReportInputs {
        collected: collected.as_deref(),
        put_back: config.pipeline.rules.then_some(&config.rules),
        reviews: reviews.as_ref(),
    })
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

/// Why a command that reads the generated docs stops before a first `generate`.
pub const NO_DOCS: &str = "no features/use cases found — run `retrodoc generate` first";

/// The generated docs the `search` and `mcp` commands answer from: the
/// artifacts of the last `generate` run and the project's own Markdown docs.
pub fn load_docs(workspace: &super::workspace::Workspace) -> anyhow::Result<retrodoc_mcp::Docs> {
    let docs = project_docs(&workspace.repo_root, &workspace.config)?;
    retrodoc_mcp::Docs::load(&workspace.repo_root, &docs)?.context(NO_DOCS)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: REC-1 (internal: the input is a generated run, which only an LLM run produces through the binary)
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

        let collected = project_docs(dir.path(), &Config::default()).unwrap();

        let paths: Vec<&Path> = collected.iter().map(|d| d.path.as_path()).collect();
        assert_eq!(paths, [Path::new("docs/guide.md")]);
    }

    // spec: REC-12 (internal: the input is a generated run, which only an LLM run produces through the binary)
    #[test]
    fn the_pages_link_what_documents_or_contradicts_the_item_only() {
        use retrodoc_pipeline::reconcile::{
            Contradiction, ItemKind, ItemVerdict, Reconciliation, SectionRef, Status,
        };
        use retrodoc_render::{DocLink, ItemRef};
        let verdict = |kind, label: &str, status, sections: &[&str]| ItemVerdict {
            kind,
            label: label.into(),
            domain: "billing".into(),
            name: label.into(),
            entry_points: 0,
            confidence: None,
            status,
            sections: sections.iter().map(|s| SectionRef::parse(s)).collect(),
            contradictions: Vec::new(),
        };
        let mut refund = verdict(
            ItemKind::UseCase,
            "billing/pay/refund",
            Status::Contradicted,
            &["docs/guide.md#Refunds"],
        );
        refund.contradictions.push(Contradiction {
            section: SectionRef::parse("docs/guide.md#Refunds"),
            doc_states: "A week.".into(),
            item_states: "At once.".into(),
        });
        let reconciliation = Reconciliation {
            items: vec![
                verdict(
                    ItemKind::Domain,
                    "billing",
                    Status::Documented,
                    &["README.md"],
                ),
                verdict(
                    ItemKind::Feature,
                    "billing/pay",
                    Status::Matched,
                    &["README.md"],
                ),
                refund,
                verdict(
                    ItemKind::Rule,
                    "billing/pay/pay-1",
                    Status::Documented,
                    &["a.md#A"],
                ),
                verdict(
                    ItemKind::Rule,
                    "billing/pay/pay-2",
                    Status::Documented,
                    &["b.md#B"],
                ),
            ],
        };
        let mut cited = retrodoc_core::model::BusinessRule {
            slug: "pay-1".into(),
            statement: "x".into(),
            kind: retrodoc_core::model::RuleKind::Limit,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: vec![retrodoc_core::model::RuleEvidence::Doc {
                path: "a.md".into(),
                heading: "A".into(),
            }],
            confidence: None,
        };
        let plain = retrodoc_core::model::BusinessRule {
            slug: "pay-2".into(),
            evidence: Vec::new(),
            ..cited.clone()
        };
        cited.statement = "y".into();

        let links = doc_links(&reconciliation, &[cited, plain], "docs");

        assert_eq!(links.docs_dir, std::path::PathBuf::from("docs"));
        let keys: Vec<&ItemRef> = links.links.keys().collect();
        assert_eq!(
            keys,
            [
                &ItemRef::Domain("billing".into()),
                &ItemRef::UseCase {
                    feature: "pay".into(),
                    use_case: "refund".into()
                },
                &ItemRef::Rule("pay-2".into()),
            ]
        );
        assert_eq!(
            links.links[&ItemRef::UseCase {
                feature: "pay".into(),
                use_case: "refund".into()
            }],
            [DocLink {
                path: "docs/guide.md".into(),
                heading: Some("Refunds".into()),
                contradicts: Some("A week.".into()),
            }]
        );
    }

    // spec: WEB-123 (internal: a run with no reconciliation needs an LLM run through the binary)
    #[test]
    fn the_docs_dir_is_set_with_no_reconciliation() {
        let mut config = Config::default();
        config.output.docs_dir = "handbook".into();
        let links = item_doc_links(None, &config);
        assert_eq!(links.docs_dir, std::path::PathBuf::from("handbook"));
        assert!(links.links.is_empty());
    }
}
