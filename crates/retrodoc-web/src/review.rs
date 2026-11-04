//! The human reviews of the sections (`specs/section-review.md`) as the pages show them: read from
//! `.retrodoc/reviews.json` at each request, so a review saved while the server runs shows on the
//! next page without a restart.

use retrodoc_mcp::reviews::{
    self, Outdated, Review, Reviews, ReviewsError, Section, SectionKind, Status,
};
use retrodoc_mcp::GeneratedDocs;

use crate::texts::ReviewTexts;
use crate::view::{find_feature, StaleFile};

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
            Self::UseCase { feature, slug, .. } => Section::use_case(feature, slug),
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

/// The statuses, in the order the form offers them, each with its key: the value the form sends
/// and the class of its badge, as the reviews file writes it.
pub const STATUSES: [(Status, &str); 3] = [
    (Status::Validated, "validated"),
    (Status::Wrong, "wrong"),
    (Status::ToReview, "to_review"),
];

/// The key of `status` ([`STATUSES`]).
#[must_use]
pub fn key(status: Status) -> &'static str {
    STATUSES
        .iter()
        .find(|(s, _)| *s == status)
        .map_or("", |(_, key)| key)
}

/// The status a form sends, `Some(None)` for the empty key (no review); `None` for a key outside
/// [`STATUSES`].
#[must_use]
pub fn parse(value: &str) -> Option<Option<Status>> {
    if value.is_empty() {
        return Some(None);
    }
    STATUSES
        .iter()
        .find(|(_, key)| *key == value)
        .map(|(status, _)| Some(*status))
}

/// The status of a review as a badge, its class the key of the status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewBadge {
    pub status: Status,
    pub class: &'static str,
}

impl ReviewBadge {
    #[must_use]
    pub fn of(status: Status) -> Self {
        Self {
            status,
            class: key(status),
        }
    }

    /// The badge of the review of the section at `place`, `None` when it has none.
    #[must_use]
    pub fn of_place(reviews: &Reviews, place: &Place) -> Option<Self> {
        reviews
            .get(&place.section())
            .map(|review| Self::of(review.status))
    }
}

/// Why a review is outdated, as text.
pub struct OutdatedView {
    pub rewritten: bool,
    pub changed: Vec<StaleFile>,
}

impl OutdatedView {
    #[must_use]
    pub fn of(outdated: &Outdated) -> Self {
        Self {
            rewritten: outdated.rewritten,
            changed: outdated
                .changed
                .iter()
                .map(|(path, state)| StaleFile::of(path, *state))
                .collect(),
        }
    }
}

/// A status the form offers, or no review.
pub struct StatusOption {
    pub key: &'static str,
    pub label: &'static str,
    pub selected: bool,
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
    /// Why the reviews cannot be read or the review cannot be saved, shown in place of the
    /// review.
    pub error: Option<String>,
    /// The status the form selects: that of the review, or the one sent by a save that failed.
    pub selected: Option<Status>,
}

impl ReviewView {
    /// The review of the section at `place` among `loaded`.
    #[must_use]
    pub fn of(loaded: &Result<Reviews, ReviewsError>, place: &Place) -> Self {
        match loaded {
            Err(err) => Self::failed(place, err.to_string(), None, String::new()),
            Ok(reviews) => Self::shown(place, reviews.get(&place.section())),
        }
    }

    /// `review` of the section at `place`, none when it has no review.
    #[must_use]
    pub fn shown(place: &Place, review: Option<&Review>) -> Self {
        let Some(review) = review else {
            return Self {
                action: place.href(),
                ..Self::default()
            };
        };
        Self {
            action: place.href(),
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
            selected: Some(review.status),
        }
    }

    /// `error` in place of the review of the section at `place`, the form holding again what was
    /// sent, `status` and `comment`, so that saving again sends the same.
    #[must_use]
    pub fn failed(place: &Place, error: String, status: Option<Status>, comment: String) -> Self {
        Self {
            action: place.href(),
            comment,
            error: Some(error),
            selected: status,
            ..Self::default()
        }
    }

    /// The choices of the form, labelled from `texts`: no review, then [`STATUSES`].
    #[must_use]
    pub fn options(&self, texts: &ReviewTexts) -> Vec<StatusOption> {
        std::iter::once(("", None))
            .chain(STATUSES.iter().map(|(status, key)| (*key, Some(*status))))
            .map(|(key, status)| StatusOption {
                key,
                label: status.map_or(texts.no_status, |status| texts.status(status)),
                selected: status == self.selected,
            })
            .collect()
    }
}
