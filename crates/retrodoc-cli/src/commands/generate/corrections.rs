//! The human corrections `generate` gives the passes (`specs/human-corrections.md`, ADR 0056): the
//! edits and the reviews of `.retrodoc/`, read through `retrodoc-docs`, matched with the sections
//! the saved results hold, before `--force` removes them.

use std::path::Path;

use anyhow::Context;
use retrodoc_core::artifacts::DomainMap;
use retrodoc_docs::edits::{self, Edit};
use retrodoc_docs::history::SectionKind;
use retrodoc_docs::reviews::{self, Review, Status};
use retrodoc_docs::store::Section;
use retrodoc_pipeline::{Correction, Corrections, Verdict};

/// The corrections of the sections the saved results hold. An edit or a review of a section they
/// do not hold, a review `to_review` and a blank field are left out (HCR-9).
///
/// A file of reviews that cannot be read is warned of, and gives no review.
///
/// # Errors
///
/// A file of edits that cannot be read or is of a later format (HCR-11).
pub(super) fn load(repo_root: &Path) -> anyhow::Result<Corrections> {
    let edits = edits::load(repo_root).with_context(|| {
        format!(
            "nothing was run: the human edits of {} cannot be read",
            edits::edits_path(repo_root).display()
        )
    })?;
    // As the run history counts them (RHI-14): warned of, and no review.
    let reviews = reviews::load(repo_root).unwrap_or_else(|err| {
        tracing::warn!(
            "the human reviews of {} cannot be read, none guides the run: {err}",
            reviews::reviews_path(repo_root).display()
        );
        reviews::Reviews::default()
    });
    let of = |section: Section, slug: &str, (name, description): (&str, &str)| {
        correction(
            slug,
            (name, description),
            edits.edits().iter().find(|e| e.section == section),
            reviews.reviews().iter().find(|r| r.section == section),
        )
    };
    let mut corrections = Corrections::default();
    for domain in DomainMap::load(repo_root).unwrap_or_default().domains {
        let section = Section::new(SectionKind::Domain, domain.slug.clone());
        corrections.add_domain(of(
            section,
            &domain.slug,
            (&domain.name, &domain.description),
        ));
    }
    for feature in retrodoc_core::artifacts::load_features(repo_root).unwrap_or_default() {
        let section = Section::new(SectionKind::Feature, feature.slug.clone());
        corrections.add_feature(
            &feature.domain_slug,
            of(
                section,
                &feature.slug,
                (&feature.name, &feature.description),
            ),
        );
    }
    for use_case in retrodoc_core::artifacts::load_use_cases(repo_root).unwrap_or_default() {
        let section = Section::use_case(&use_case.feature_slug, &use_case.slug);
        corrections.add_use_case(
            &use_case.feature_slug,
            of(
                section,
                &use_case.slug,
                (&use_case.name, &use_case.description),
            ),
        );
    }
    Ok(corrections)
}

/// The correction of the section `slug`, generated with `name` and `description`, from its edit
/// and its review.
fn correction(
    slug: &str,
    (name, description): (&str, &str),
    edit: Option<&Edit>,
    review: Option<&Review>,
) -> Correction {
    let field = |get: fn(&edits::Fields) -> &Option<String>| {
        edit.and_then(|e| get(&e.fields).clone())
            .filter(|text| !text.trim().is_empty())
    };
    let edited_name = field(|f| &f.name);
    let edited_description = field(|f| &f.description);
    Correction {
        slug: slug.to_string(),
        shown_name: edited_name.clone().unwrap_or_else(|| name.to_string()),
        shown_description: edited_description
            .clone()
            .unwrap_or_else(|| description.to_string()),
        name: edited_name,
        description: edited_description,
        narrative: field(|f| &f.narrative),
        verdict: review.and_then(verdict),
    }
}

/// What a review asks of the run: nothing for a review `to_review`.
fn verdict(review: &Review) -> Option<Verdict> {
    match review.status {
        Status::Wrong => Some(Verdict::Wrong {
            comment: review.comment.clone(),
            reviewed_at: review.reviewed_at.clone(),
        }),
        Status::Validated => Some(Verdict::Validated),
        Status::ToReview => None,
    }
}
