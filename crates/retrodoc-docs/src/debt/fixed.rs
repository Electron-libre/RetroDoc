//! The sections a human settled (`specs/debt-fixed.md`): reviewed, validated or wrong, and the
//! review up to date or followed by an edit up to date. They leave the items left of the debt for
//! a list of their own, read from the saved reviews and edits, never recorded.

use chrono::{DateTime, FixedOffset};

use crate::edits;
use crate::read_model::GeneratedDocs;
use crate::reviews::{self, Reviews, Status};

use super::{section_subject, DebtKind, Subject};

/// A section a human settled, and the kinds of the items it took out of the debt.
#[derive(Debug, Clone, PartialEq)]
pub struct Fixed {
    /// The section of its review.
    pub section: reviews::Section,
    pub subject: Subject,
    /// `Validated` or `Wrong`.
    pub status: Status,
    /// The comment of the review, empty when none was given.
    pub comment: String,
    /// The author of the review.
    pub author: Option<String>,
    /// RFC 3339, UTC, to the second.
    pub reviewed_at: String,
    /// The author and the date of the edit saved at or after the review that keeps it settled.
    pub edit: Option<(Option<String>, String)>,
    /// The kinds of the items it took out of the items left, in the order of [`DebtKind::ALL`].
    pub kinds: Vec<DebtKind>,
}

impl Fixed {
    /// The date of its review, or of its edit when later.
    #[must_use]
    pub fn date(&self) -> &str {
        self.edit
            .as_ref()
            .map_or(self.reviewed_at.as_str(), |(_, at)| at.as_str())
    }

    /// The label of its section ([`Subject::label`]).
    #[must_use]
    pub fn label(&self) -> String {
        self.subject.label()
    }
}

/// The sections of the run a human settled (FIX-1), the newest first then by section (FIX-7), no
/// kind of judgement taken out yet: a review validated or wrong, up to date or followed by an edit
/// up to date.
pub(super) fn settled(docs: &GeneratedDocs, reviews: &Reviews) -> Vec<Fixed> {
    let mut fixed: Vec<Fixed> = reviews
        .reviews()
        .iter()
        .filter(|r| r.status != Status::ToReview)
        .filter_map(|review| {
            // The subject first: no file read for a section the run no longer holds.
            let subject = section_subject(docs, &review.section)?;
            let edit = docs.edits().get(&review.section).filter(|edit| {
                !edit.changed().is_empty()
                    && matches!(
                        (instant(&edit.edited_at), instant(&review.reviewed_at)),
                        (Some(edited), Some(reviewed)) if edited >= reviewed
                    )
                    && edits::outdated(docs, edit).is_none()
            });
            (edit.is_some() || reviews::outdated(docs, review).is_none()).then(|| Fixed {
                section: review.section.clone(),
                subject,
                status: review.status,
                comment: review.comment.clone(),
                author: review.author.clone(),
                reviewed_at: review.reviewed_at.clone(),
                edit: edit.map(|e| (e.author.clone(), e.edited_at.clone())),
                // Its review would list it marked wrong (DBT-30); the kinds of judgement are
                // added as they are taken out.
                kinds: if review.status == Status::Wrong {
                    vec![DebtKind::MarkedWrong]
                } else {
                    Vec::new()
                },
            })
        })
        .collect();
    // A date that does not parse (a file written by hand) comes last.
    fixed.sort_by(|a, b| {
        instant(b.date())
            .cmp(&instant(a.date()))
            .then_with(|| a.section.cmp(&b.section))
    });
    fixed
}

/// The instant of an RFC 3339 date, whatever its offset; `None` when it does not parse.
fn instant(date: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(date).ok()
}

#[cfg(test)]
#[path = "fixed_tests.rs"]
mod tests;
