//! What the pages show, computed from the read model: plain data the
//! templates only lay out, so the rules of what a page shows are tested
//! without HTML.

use std::collections::BTreeSet;

use retrodoc_core::model::{ConfidenceScore, Feature, LOW_CONFIDENCE_THRESHOLD};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_render::{anchor, kind_label, DocLinks, ItemRef};

/// The docs the interface serves, `None` before a first `generate`, and the
/// tree every page shows, built once since the docs don't change.
pub struct Site {
    docs: Option<GeneratedDocs>,
    links: DocLinks,
    tree: Vec<TreeDomain>,
}

impl Site {
    #[must_use]
    pub fn new(docs: Option<GeneratedDocs>, links: DocLinks) -> Self {
        let tree = docs.as_ref().map(tree).unwrap_or_default();
        Self { docs, links, tree }
    }

    #[must_use]
    pub fn docs(&self) -> Option<&GeneratedDocs> {
        self.docs.as_ref()
    }

    #[must_use]
    pub fn links(&self) -> &DocLinks {
        &self.links
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
/// [`LOW_CONFIDENCE_THRESHOLD`], with why the model gave it.
pub struct Confidence {
    pub label: String,
    pub low: bool,
    pub rationale: Option<String>,
}

impl Confidence {
    #[must_use]
    pub fn of(score: Option<&ConfidenceScore>) -> Self {
        match score {
            None => Self {
                label: "not scored".into(),
                low: false,
                rationale: None,
            },
            Some(s) => Self {
                label: format!("{:.0}%", s.value * 100.0),
                low: s.value < LOW_CONFIDENCE_THRESHOLD,
                rationale: s
                    .rationale
                    .as_deref()
                    .map(str::trim)
                    .filter(|r| !r.is_empty())
                    .map(str::to_string),
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

/// A link of a breadcrumb.
pub struct Link {
    pub href: String,
    pub name: String,
}

/// A feature or a use case in the list of the page above it.
pub struct ItemRow {
    pub href: String,
    pub name: String,
    pub confidence: Confidence,
    pub description: String,
}

/// A rule in the list of a domain, with the feature it belongs to.
pub struct DomainRule {
    pub statement: String,
    pub kind: &'static str,
    pub confidence: Confidence,
    pub feature: Link,
}

/// A section of the project's own docs that documents or contradicts an item.
pub struct DocItem {
    /// The page of the collected doc, at the section.
    pub href: String,
    /// From the repository root.
    pub path: String,
    pub heading: Option<String>,
    /// What the section states against the item.
    pub contradicts: Option<String>,
}

/// The page of a domain, its sections in the order of its Markdown page.
pub struct DomainPage {
    pub crumbs: Vec<Link>,
    pub name: String,
    pub description: String,
    pub confidence: Confidence,
    pub features: Vec<ItemRow>,
    /// The back-office features, set apart.
    pub back_office: Vec<ItemRow>,
    pub rules: Vec<DomainRule>,
    pub docs: Vec<DocItem>,
}

fn home_crumb() -> Link {
    Link {
        href: "/".into(),
        name: "Domains".into(),
    }
}

fn feature_href(feature: &Feature) -> String {
    format!("/domains/{}/{}", feature.domain_slug, feature.slug)
}

fn feature_row(feature: &Feature) -> ItemRow {
    ItemRow {
        href: feature_href(feature),
        name: feature.name.clone(),
        confidence: Confidence::of(feature.confidence.as_ref()),
        description: feature.description.trim().to_string(),
    }
}

/// The doc sections behind `item`, linked to the page of the collected doc.
fn doc_items(links: &DocLinks, item: &ItemRef) -> Vec<DocItem> {
    links
        .of(item)
        .iter()
        .map(|doc| DocItem {
            href: match &doc.heading {
                Some(h) => format!("/docs/{}#{}", doc.path, anchor(h)),
                None => format!("/docs/{}", doc.path),
            },
            path: doc.path.clone(),
            heading: doc.heading.clone(),
            contradicts: doc.contradicts.clone(),
        })
        .collect()
}

/// The page of the domain `slug`, `None` when the run has no such domain.
#[must_use]
pub fn domain_page(docs: &GeneratedDocs, links: &DocLinks, slug: &str) -> Option<DomainPage> {
    let domain = docs.domains().iter().find(|d| d.slug == slug)?;
    let features: Vec<&Feature> = docs.features_of(slug).collect();
    let (back_office, business): (Vec<&Feature>, Vec<&Feature>) =
        features.iter().partition(|f| f.back_office);
    let rules = features
        .iter()
        .flat_map(|f| {
            docs.rules()
                .iter()
                .filter(|r| r.belongs_to_feature(&f.slug))
                .map(move |r| DomainRule {
                    statement: r.statement.clone(),
                    kind: kind_label(r.kind),
                    confidence: Confidence::of(r.confidence.as_ref()),
                    feature: Link {
                        href: feature_href(f),
                        name: f.name.clone(),
                    },
                })
        })
        .collect();
    Some(DomainPage {
        crumbs: vec![home_crumb()],
        name: domain.name.clone(),
        description: domain.description.trim().to_string(),
        confidence: Confidence::of(domain.confidence.as_ref()),
        features: business.into_iter().map(feature_row).collect(),
        back_office: back_office.into_iter().map(feature_row).collect(),
        rules,
        docs: doc_items(links, &ItemRef::Domain(slug.to_string())),
    })
}
