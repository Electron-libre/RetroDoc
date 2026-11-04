//! What the detail answers say of the human review of their section (`specs/section-review.md`),
//! read from `.retrodoc/reviews.json` at each answer, so a review saved while the server runs shows
//! without a restart.

use std::fmt::Write as _;

use retrodoc_core::text::one_line;

use super::Docs;
use crate::reviews::{self, Outdated, Review, Reviews, Section, Status};

/// The review of a section as an answer shows it.
pub(super) struct ReviewNote {
    /// The paragraph that opens the answer of a section marked wrong; empty otherwise.
    pub warning: String,
    /// `Review: …`, on the line after the confidence.
    pub line: String,
}

impl Docs {
    /// The review of `section`; without a repository, the docs read as holding no review.
    pub(super) fn review_note(&self, section: &Section) -> ReviewNote {
        let loaded = self
            .generated
            .repo_root()
            .map_or_else(|| Ok(Reviews::default()), reviews::load);
        match loaded {
            Ok(reviews) => match reviews.get(section) {
                Some(review) => {
                    let outdated = reviews::outdated(&self.generated, review);
                    ReviewNote {
                        warning: wrong_warning(review, outdated.as_ref()),
                        line: review_line(review, outdated.as_ref()),
                    }
                }
                None => ReviewNote {
                    warning: String::new(),
                    line: "Review: not reviewed".into(),
                },
            },
            Err(err) => ReviewNote {
                warning: String::new(),
                line: format!("Review: unavailable, {}", one_line(&err.to_string())),
            },
        }
    }
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Validated => "validated",
        Status::Wrong => "wrong",
        Status::ToReview => "to review",
    }
}

/// `Review: wrong — "comment" (author, date), outdated: <reasons>`, the comment, the author and
/// the reasons left out when absent.
fn review_line(review: &Review, outdated: Option<&Outdated>) -> String {
    let mut line = format!("Review: {}", status_label(review.status));
    let comment = one_line(&review.comment);
    if !comment.is_empty() {
        let _ = write!(line, " — \"{comment}\"");
    }
    match &review.author {
        Some(author) => {
            let _ = write!(
                line,
                " ({}, {})",
                one_line(author),
                one_line(&review.reviewed_at)
            );
        }
        None => {
            let _ = write!(line, " ({})", one_line(&review.reviewed_at));
        }
    }
    if let Some(outdated) = outdated {
        let _ = write!(line, ", outdated: {}", outdated.reasons());
    }
    line
}

fn wrong_warning(review: &Review, outdated: Option<&Outdated>) -> String {
    if review.status != Status::Wrong {
        return String::new();
    }
    let comment = one_line(&review.comment);
    let quoted = if comment.is_empty() {
        String::new()
    } else {
        format!(": \"{comment}\"")
    };
    let advice = match outdated {
        None => "Do not rely on it.".to_string(),
        Some(outdated) => format!(
            "The section changed since ({}): check whether it still holds before relying on it.",
            outdated.reasons()
        ),
    };
    format!("⚠ Marked wrong: a human reviewer marked this section wrong{quoted}. {advice}\n\n")
}
