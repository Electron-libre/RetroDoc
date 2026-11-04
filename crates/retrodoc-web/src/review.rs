//! The human reviews of the sections (`specs/section-review.md`) as the pages show them: read from
//! `.retrodoc/reviews.json` at each request, so a review saved while the server runs shows on the
//! next page without a restart.

use retrodoc_mcp::reviews::{self, Reviews, ReviewsError, Section, Status};
use retrodoc_mcp::GeneratedDocs;

/// The reviews of the repository of `docs`, none when the docs carry no repository.
///
/// # Errors
///
/// The file cannot be read or is of a later format.
pub fn load(docs: &GeneratedDocs) -> Result<Reviews, ReviewsError> {
    docs.repo_root()
        .map_or_else(|| Ok(Reviews::default()), reviews::load)
}

/// The status of a review as a badge, its class the key of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewBadge {
    pub status: Status,
    pub class: &'static str,
}

impl ReviewBadge {
    #[must_use]
    pub fn of(status: Status) -> Self {
        let class = match status {
            Status::Validated => "validated",
            Status::Wrong => "wrong",
            Status::ToReview => "to_review",
        };
        Self { status, class }
    }

    /// The badge of the review of `section`, `None` when it has none.
    #[must_use]
    pub fn of_section(reviews: &Reviews, section: &Section) -> Option<Self> {
        reviews.get(section).map(|review| Self::of(review.status))
    }
}

/// The review of a section on its page.
#[derive(Debug, Default)]
pub struct ReviewView {
    /// `None` when the section has no review.
    pub badge: Option<ReviewBadge>,
    pub comment: String,
    pub author: Option<String>,
    pub date: String,
    /// Why the reviews cannot be read, shown in place of the review.
    pub error: Option<String>,
}

impl ReviewView {
    /// The review of `section` among `loaded`.
    #[must_use]
    pub fn of(loaded: &Result<Reviews, ReviewsError>, section: &Section) -> Self {
        match loaded {
            Err(err) => Self {
                error: Some(err.to_string()),
                ..Self::default()
            },
            Ok(reviews) => reviews
                .get(section)
                .map_or_else(Self::default, |review| Self {
                    badge: Some(ReviewBadge::of(review.status)),
                    comment: review.comment.trim().to_string(),
                    author: review
                        .author
                        .as_deref()
                        .map(str::trim)
                        .filter(|author| !author.is_empty())
                        .map(str::to_string),
                    date: review.reviewed_at.clone(),
                    error: None,
                }),
        }
    }
}
