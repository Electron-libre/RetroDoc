//! The human reviews of the sections (`specs/section-review.md`) as the pages show them: read from
//! `.retrodoc/reviews.json` at each request, so a review saved while the server runs shows on the
//! next page without a restart.

use retrodoc_mcp::reviews::{self, Reviews, ReviewsError, Section, SectionKind, Status};
use retrodoc_mcp::GeneratedDocs;

use crate::view::find_feature;

/// The reviews of the repository of `docs`, none when the docs carry no repository.
///
/// # Errors
///
/// The file cannot be read or is of a later format.
pub fn load(docs: &GeneratedDocs) -> Result<Reviews, ReviewsError> {
    docs.repo_root()
        .map_or_else(|| Ok(Reviews::default()), reviews::load)
}

/// A section as its page names it: by its domain, and its feature for a use case.
#[derive(Debug, Clone)]
pub enum Place {
    Domain(String),
    Feature {
        domain: String,
        slug: String,
    },
    UseCase {
        domain: String,
        feature: String,
        slug: String,
    },
}

impl Place {
    /// The section the reviews name (REV-1).
    #[must_use]
    pub fn section(&self) -> Section {
        match self {
            Self::Domain(slug) => Section::new(SectionKind::Domain, slug),
            Self::Feature { slug, .. } => Section::new(SectionKind::Feature, slug),
            Self::UseCase { feature, slug, .. } => {
                Section::new(SectionKind::UseCase, format!("{feature}/{slug}"))
            }
        }
    }

    /// The URL of its page, where its form posts.
    #[must_use]
    pub fn href(&self) -> String {
        match self {
            Self::Domain(slug) => format!("/domains/{slug}"),
            Self::Feature { domain, slug } => format!("/domains/{domain}/{slug}"),
            Self::UseCase {
                domain,
                feature,
                slug,
            } => format!("/domains/{domain}/{feature}/{slug}"),
        }
    }

    /// Whether `docs` hold it at that place: a feature in its domain, a use case in its feature.
    #[must_use]
    pub fn in_docs(&self, docs: &GeneratedDocs) -> bool {
        match self {
            Self::Domain(slug) => docs.domains().iter().any(|d| &d.slug == slug),
            Self::Feature { domain, slug } => find_feature(docs, domain, slug).is_some(),
            Self::UseCase {
                domain,
                feature,
                slug,
            } => {
                find_feature(docs, domain, feature).is_some()
                    && docs.use_cases_of(feature).any(|u| &u.slug == slug)
            }
        }
    }
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

/// The review of a section on its page, with the form that changes it.
#[derive(Debug, Default)]
pub struct ReviewView {
    /// Where the form posts: the page of the section.
    pub action: String,
    /// `None` when the section has no review.
    pub badge: Option<ReviewBadge>,
    pub comment: String,
    pub author: Option<String>,
    pub date: String,
    /// Why the reviews cannot be read, shown in place of the review.
    pub error: Option<String>,
}

impl ReviewView {
    /// The review of the section at `place` among `loaded`.
    #[must_use]
    pub fn of(loaded: &Result<Reviews, ReviewsError>, place: &Place) -> Self {
        let action = place.href();
        match loaded {
            Err(err) => Self::failed(action, err.to_string(), String::new()),
            Ok(reviews) => reviews.get(&place.section()).map_or_else(
                || Self {
                    action: action.clone(),
                    ..Self::default()
                },
                |review| Self {
                    action: action.clone(),
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
                },
            ),
        }
    }

    /// `error` in place of the review of the section whose page is `action`, the form holding
    /// `comment` again.
    #[must_use]
    pub fn failed(action: String, error: String, comment: String) -> Self {
        Self {
            action,
            comment,
            error: Some(error),
            ..Self::default()
        }
    }

    /// Whether the form selects the status of the key `value`, `""` for none.
    #[must_use]
    pub fn selects(&self, value: &str) -> bool {
        self.badge.map_or("", |badge| badge.class) == value
    }
}
