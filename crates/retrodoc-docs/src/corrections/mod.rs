//! One save of the form of a section (`specs/section-form.md`): its human edit, then its review,
//! the review fingerprinted on the section with that edit applied, so that the edit saved with it
//! does not make it outdated (SFM-4, SED-26). Both files are read before either is written, so a
//! refusal or a file that cannot be read leaves both as they were (SFM-13, SFM-14).

use std::path::Path;

use crate::edits::{self, Edit, EditsError, Fields};
use crate::reviews::{self, Review, ReviewsError, Section, Status};
use crate::GeneratedDocs;

/// What a save does to the edit of a section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditChange {
    /// Left as it is, an outdated edit included.
    Keep,
    /// Replaced by an edit of these fields.
    Set(Fields),
    /// Removed: the section shows the generated text.
    Remove,
}

/// What the form of a section sends, once read: the change of its edit and its review, `None`
/// removing the review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    pub edit: EditChange,
    pub review: Option<(Status, String)>,
}

/// The edit and the review of the section once saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub edit: Option<Edit>,
    pub review: Option<Review>,
}

#[derive(Debug, thiserror::Error)]
pub enum CorrectionError {
    #[error(transparent)]
    Edits(#[from] EditsError),
    #[error(transparent)]
    Reviews(#[from] ReviewsError),
}

impl CorrectionError {
    /// Whether the docs lack the section.
    #[must_use]
    pub fn is_unknown_section(&self) -> bool {
        matches!(
            self,
            Self::Edits(EditsError::UnknownSection(_))
                | Self::Reviews(ReviewsError::UnknownSection(_))
        )
    }
}

/// Saves `correction` of `section` in `docs`: its edit, then its review on the section as the
/// readers will show it with that edit.
///
/// # Errors
///
/// The section is not in `docs`, the fields are refused (`edits::record`), or a file cannot be
/// read, is of a later format or cannot be written; nothing is written but for a write that fails
/// after the edit was saved.
pub fn save(
    repo_root: &Path,
    docs: &GeneratedDocs,
    section: &Section,
    correction: &Correction,
) -> Result<Saved, CorrectionError> {
    if reviews::fingerprint(docs, section).is_none() {
        return Err(ReviewsError::UnknownSection(section.clone()).into());
    }
    if let EditChange::Set(fields) = &correction.edit {
        edits::check(section, fields)?;
    }
    edits::load(repo_root)?;
    reviews::load(repo_root)?;
    let mut shown = docs.edits().clone();
    match &correction.edit {
        EditChange::Keep => {}
        EditChange::Set(fields) => {
            shown.set(edits::record(repo_root, docs, section, fields.clone())?);
        }
        EditChange::Remove => {
            edits::remove(repo_root, section)?;
            shown.unset(section);
        }
    }
    let sections = edits::apply(docs.generated().clone(), &shown);
    let review = if let Some((status, comment)) = &correction.review {
        Some(reviews::record_in(
            repo_root, docs, &sections, section, *status, comment,
        )?)
    } else {
        reviews::remove(repo_root, section)?;
        None
    };
    Ok(Saved {
        edit: shown.get(section).cloned(),
        review,
    })
}

#[cfg(test)]
mod tests;
