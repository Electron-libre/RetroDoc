use std::path::Path;

use retrodoc_core::model::{Feature, UseCase};
use retrodoc_mcp::reviews::{self, Review, SectionKind, Status};
use retrodoc_pipeline::reconcile::ItemKind;
use retrodoc_pipeline::{DomainMap, MarkedWrong};

use super::workspace::repo_root;

/// Prints the documentation debt report from the artifacts saved by the last
/// `generate` run (`.retrodoc/cache/`); makes no LLM call.
pub fn run(path: &Path) -> anyhow::Result<()> {
    let repo_root = repo_root(path)?;
    // The report needs no `retrodoc.toml`: without one, the default document paths. A broken one
    // is an error, not a silent change of which docs are compared.
    let config = if repo_root
        .join(retrodoc_core::config::CONFIG_FILE_NAME)
        .exists()
    {
        retrodoc_core::config::Config::load(&repo_root)?
    } else {
        retrodoc_core::config::Config::default()
    };

    let (Some(features), Some(use_cases)) = (
        retrodoc_pipeline::load_features(&repo_root),
        retrodoc_pipeline::load_use_cases(&repo_root),
    ) else {
        anyhow::bail!("no features/use cases found — run `retrodoc generate` first");
    };
    if use_cases.iter().all(|u| u.confidence.is_none()) {
        anyhow::bail!("no confidence scores found — run `retrodoc generate` again");
    }
    let domains = DomainMap::load(&repo_root).unwrap_or_default();

    let mut report = retrodoc_pipeline::build_report(&domains, &features, &use_cases);
    report.skipped_files = retrodoc_pipeline::Scope::load(&repo_root)
        .map(|s| s.skipped)
        .unwrap_or_default();
    report.with_behaviour_coverage(retrodoc_pipeline::BehaviourCoverage::load(&repo_root));
    report.with_technical_features(retrodoc_pipeline::load_technical_features(&repo_root));
    super::docs::add_doc_debt(
        &mut report,
        &repo_root,
        &config,
        &domains,
        &features,
        &use_cases,
    );
    match reviews::load(&repo_root) {
        Ok(reviews) => {
            report.marked_wrong = reviews
                .reviews()
                .iter()
                .filter(|r| r.status == Status::Wrong)
                .filter_map(|r| marked_wrong(r, &features, &use_cases))
                .collect();
        }
        Err(err) => tracing::warn!("the report lists no section marked wrong: {err}"),
    }
    print!("{}", report.to_markdown());
    Ok(())
}

/// The section `review` names, with its comment; `None` when the run no longer holds it. A domain
/// is in the run while a feature belongs to it, as the docs show it.
fn marked_wrong(
    review: &Review,
    features: &[Feature],
    use_cases: &[UseCase],
) -> Option<MarkedWrong> {
    let id = review.section.id.as_str();
    let feature = |slug: &str| features.iter().find(|f| f.slug == slug);
    let (kind, label) = match review.section.kind {
        SectionKind::Domain => {
            features.iter().find(|f| f.domain_slug == id)?;
            (ItemKind::Domain, id.to_string())
        }
        SectionKind::Feature => {
            let feature = feature(id)?;
            (ItemKind::Feature, format!("{}/{id}", feature.domain_slug))
        }
        SectionKind::UseCase => {
            let (slug, use_case) = id.split_once('/')?;
            let feature = feature(slug)?;
            use_cases
                .iter()
                .find(|u| u.feature_slug == slug && u.slug == use_case)?;
            (ItemKind::UseCase, format!("{}/{id}", feature.domain_slug))
        }
    };
    Some(MarkedWrong {
        kind,
        label,
        comment: review.comment.clone(),
    })
}
