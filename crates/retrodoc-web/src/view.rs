//! What the pages show, computed from the read model: plain data the
//! templates only lay out, so the rules of what a page shows are tested
//! without HTML.

use std::collections::BTreeSet;

use retrodoc_core::model::{ConfidenceScore, LOW_CONFIDENCE_THRESHOLD};
use retrodoc_mcp::GeneratedDocs;

/// The docs the interface serves, `None` before a first `generate`, and the
/// tree every page shows, built once since the docs don't change.
pub struct Site {
    docs: Option<GeneratedDocs>,
    tree: Vec<TreeDomain>,
}

impl Site {
    #[must_use]
    pub fn new(docs: Option<GeneratedDocs>) -> Self {
        let tree = docs.as_ref().map(tree).unwrap_or_default();
        Self { docs, tree }
    }

    #[must_use]
    pub fn docs(&self) -> Option<&GeneratedDocs> {
        self.docs.as_ref()
    }

    /// The tree of every page: empty with no run.
    #[must_use]
    pub fn tree(&self) -> &[TreeDomain] {
        &self.tree
    }
}

pub struct TreeDomain {
    pub slug: String,
    pub name: String,
    pub features: Vec<TreeFeature>,
}

pub struct TreeFeature {
    pub slug: String,
    pub name: String,
    pub use_cases: Vec<TreeUseCase>,
}

pub struct TreeUseCase {
    pub slug: String,
    pub name: String,
}

/// The domains, their features and the use cases of each, in the order of the run.
#[must_use]
pub fn tree(docs: &GeneratedDocs) -> Vec<TreeDomain> {
    docs.domains()
        .iter()
        .map(|domain| TreeDomain {
            slug: domain.slug.clone(),
            name: domain.name.clone(),
            features: docs
                .features_of(&domain.slug)
                .map(|feature| TreeFeature {
                    slug: feature.slug.clone(),
                    name: feature.name.clone(),
                    use_cases: docs
                        .use_cases_of(&feature.slug)
                        .map(|u| TreeUseCase {
                            slug: u.slug.clone(),
                            name: u.name.clone(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

/// A confidence score as a badge: `80%` or `not scored`, flagged low under
/// [`LOW_CONFIDENCE_THRESHOLD`].
pub struct Confidence {
    pub label: String,
    pub low: bool,
}

impl Confidence {
    #[must_use]
    pub fn of(score: Option<&ConfidenceScore>) -> Self {
        match score {
            None => Self {
                label: "not scored".into(),
                low: false,
            },
            Some(s) => Self {
                label: format!("{:.0}%", s.value * 100.0),
                low: s.value < LOW_CONFIDENCE_THRESHOLD,
            },
        }
    }
}

/// A domain in the list of the home page.
pub struct DomainRow {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub confidence: Confidence,
    /// A file one of its features cites was modified or deleted since the run.
    pub stale: bool,
    pub features: usize,
    pub use_cases: usize,
}

/// Every domain of the run, in its order. Reads the files the features cite
/// to tell whether they changed since the run.
#[must_use]
pub fn domain_rows(docs: &GeneratedDocs) -> Vec<DomainRow> {
    docs.domains()
        .iter()
        .map(|domain| {
            let features: Vec<_> = docs.features_of(&domain.slug).collect();
            let cited: BTreeSet<&str> = features
                .iter()
                .flat_map(|f| &f.source_paths)
                .map(String::as_str)
                .collect();
            let stale = docs
                .freshness()
                .is_some_and(|freshness| !freshness.stale(cited).is_empty());
            DomainRow {
                slug: domain.slug.clone(),
                name: domain.name.clone(),
                description: domain.description.clone(),
                confidence: Confidence::of(domain.confidence.as_ref()),
                stale,
                features: features.len(),
                use_cases: features
                    .iter()
                    .map(|f| docs.use_cases_of(&f.slug).count())
                    .sum(),
            }
        })
        .collect()
}
