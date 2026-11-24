//! The human corrections `generate` gives the passes (`specs/human-corrections.md`, ADR 0056): the
//! edits and the reviews of `.retrodoc/`, read through `retrodoc-docs`, matched with the sections
//! the saved results hold, before `--force` removes them.

use std::path::Path;

use anyhow::Context;
use retrodoc_core::model::UseCase;
use retrodoc_docs::edits::{self, Edit};
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
    let mut corrections = Corrections::default();
    for use_case in retrodoc_core::artifacts::load_use_cases(repo_root).unwrap_or_default() {
        let section = Section::use_case(&use_case.feature_slug, &use_case.slug);
        corrections.add_use_case(
            &use_case.feature_slug,
            of_use_case(
                &use_case,
                edits.edits().iter().find(|e| e.section == section),
                reviews.reviews().iter().find(|r| r.section == section),
            ),
        );
    }
    Ok(corrections)
}

/// The correction of `use_case` from its edit and its review.
fn of_use_case(use_case: &UseCase, edit: Option<&Edit>, review: Option<&Review>) -> Correction {
    let field = |get: fn(&edits::Fields) -> &Option<String>| {
        edit.and_then(|e| get(&e.fields).clone())
            .filter(|text| !text.trim().is_empty())
    };
    let name = field(|f| &f.name);
    let description = field(|f| &f.description);
    Correction {
        slug: use_case.slug.clone(),
        shown_name: name.clone().unwrap_or_else(|| use_case.name.clone()),
        shown_description: description
            .clone()
            .unwrap_or_else(|| use_case.description.clone()),
        name,
        description,
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
