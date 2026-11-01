//! What the pages show, computed from the read model: plain data the
//! templates only lay out, so the rules of what a page shows are tested
//! without HTML.

use std::collections::BTreeSet;

use retrodoc_core::model::{
    ActorKind, BusinessRule, ConfidenceScore, Feature, RuleEvidence, Step, UseCase,
    LOW_CONFIDENCE_THRESHOLD,
};
use retrodoc_mcp::freshness::{FileState, Freshness};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_render::{anchor, kind_label, source_ref_label, DocLinks, ItemRef};

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

fn domain_href(slug: &str) -> String {
    format!("/domains/{slug}")
}

fn feature_href(feature: &Feature) -> String {
    format!("/domains/{}/{}", feature.domain_slug, feature.slug)
}

fn use_case_href(feature: &Feature, use_case: &UseCase) -> String {
    format!("{}/{}", feature_href(feature), use_case.slug)
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

/// A file a page cites that changed since the run.
pub struct StaleFile {
    pub path: String,
    /// `modified` or `deleted`.
    pub state: &'static str,
}

/// The files among `paths` modified or deleted since the run; none when the
/// docs carry no freshness. Reads the files.
fn stale_files<'a>(
    freshness: Option<&Freshness>,
    paths: impl IntoIterator<Item = &'a str>,
) -> Vec<StaleFile> {
    freshness
        .map(|f| f.stale(paths))
        .unwrap_or_default()
        .into_iter()
        .map(|(path, state)| StaleFile {
            path: path.to_string(),
            state: if state == FileState::Deleted {
                "deleted"
            } else {
                "modified"
            },
        })
        .collect()
}

/// What backs a rule: `Code` and the cited lines, `Test` and its phrase,
/// `Documentation` and its section, with the file it comes from.
pub struct Evidence {
    pub kind: &'static str,
    pub text: String,
    pub source: Option<String>,
}

impl Evidence {
    fn of(evidence: &RuleEvidence) -> Self {
        match evidence {
            RuleEvidence::Code { source } => Self {
                kind: "Code",
                text: source_ref_label(source),
                source: None,
            },
            RuleEvidence::Test { phrase, path } => Self {
                kind: "Test",
                text: phrase.clone(),
                source: Some(path.clone()),
            },
            RuleEvidence::Doc { path, heading } => Self {
                kind: "Documentation",
                text: heading.clone(),
                source: Some(path.clone()),
            },
        }
    }
}

/// A rule on the page of its feature or of a use case it applies to.
pub struct RuleView {
    pub statement: String,
    pub kind: &'static str,
    pub confidence: Confidence,
    pub entities: Vec<String>,
    /// The use cases of the page it applies to; none on a use case page.
    pub use_cases: Vec<Link>,
    pub evidence: Vec<Evidence>,
    pub docs: Vec<DocItem>,
}

fn rule_view(
    rule: &BusinessRule,
    feature: &Feature,
    use_cases: &[&UseCase],
    links: &DocLinks,
) -> RuleView {
    RuleView {
        statement: rule.statement.clone(),
        kind: kind_label(rule.kind),
        confidence: Confidence::of(rule.confidence.as_ref()),
        entities: rule.entities.clone(),
        use_cases: use_cases
            .iter()
            .filter(|u| rule.use_cases.contains(&u.slug))
            .map(|u| Link {
                href: use_case_href(feature, u),
                name: u.name.clone(),
            })
            .collect(),
        evidence: rule.evidence.iter().map(Evidence::of).collect(),
        docs: doc_items(links, &ItemRef::Rule(rule.slug.clone())),
    }
}

/// The page of a feature, its sections in the order of its Markdown page,
/// then its entry points.
pub struct FeaturePage {
    pub crumbs: Vec<Link>,
    pub name: String,
    pub description: String,
    pub confidence: Confidence,
    pub back_office: bool,
    pub stale: Vec<StaleFile>,
    /// None at all: none could be derived from the code.
    pub no_use_case: bool,
    pub use_cases: Vec<ItemRow>,
    /// The use cases `UseCase::is_technical` sets apart.
    pub technical: Vec<ItemRow>,
    pub rules: Vec<RuleView>,
    pub docs: Vec<DocItem>,
    pub source_files: Vec<String>,
    pub entry_points: Vec<String>,
}

fn crumbs_to(docs: &GeneratedDocs, feature: Option<&Feature>, domain_slug: &str) -> Vec<Link> {
    let domain_name = docs
        .domains()
        .iter()
        .find(|d| d.slug == domain_slug)
        .map_or_else(|| domain_slug.to_string(), |d| d.name.clone());
    let mut crumbs = vec![
        home_crumb(),
        Link {
            href: domain_href(domain_slug),
            name: domain_name,
        },
    ];
    if let Some(f) = feature {
        crumbs.push(Link {
            href: feature_href(f),
            name: f.name.clone(),
        });
    }
    crumbs
}

/// The feature `slug` of the domain `domain`, `None` when the run has no such
/// feature in that domain.
fn find_feature<'a>(docs: &'a GeneratedDocs, domain: &str, slug: &str) -> Option<&'a Feature> {
    docs.features()
        .iter()
        .find(|f| f.slug == slug && f.domain_slug == domain)
}

/// The page of the feature `slug` of the domain `domain`, `None` when the run
/// has no such feature there. Reads the files it cites to tell whether they
/// changed since the run.
#[must_use]
pub fn feature_page(
    docs: &GeneratedDocs,
    links: &DocLinks,
    domain: &str,
    slug: &str,
) -> Option<FeaturePage> {
    let feature = find_feature(docs, domain, slug)?;
    let use_cases: Vec<&UseCase> = docs.use_cases_of(slug).collect();
    let row = |u: &&UseCase| ItemRow {
        href: use_case_href(feature, u),
        name: u.name.clone(),
        confidence: Confidence::of(u.confidence.as_ref()),
        description: u.description.trim().to_string(),
    };
    let (technical, business): (Vec<&UseCase>, Vec<&UseCase>) = use_cases
        .iter()
        .partition(|u| u.is_technical(feature.back_office));
    Some(FeaturePage {
        crumbs: crumbs_to(docs, None, domain),
        name: feature.name.clone(),
        description: feature.description.trim().to_string(),
        confidence: Confidence::of(feature.confidence.as_ref()),
        back_office: feature.back_office,
        stale: stale_files(
            docs.freshness(),
            feature.source_paths.iter().map(String::as_str),
        ),
        no_use_case: use_cases.is_empty(),
        use_cases: business.iter().map(row).collect(),
        technical: technical.iter().map(row).collect(),
        rules: docs
            .rules()
            .iter()
            .filter(|r| r.belongs_to_feature(slug))
            .map(|r| rule_view(r, feature, &use_cases, links))
            .collect(),
        docs: doc_items(links, &ItemRef::Feature(slug.to_string())),
        source_files: feature.source_paths.clone(),
        entry_points: feature.entry_points.clone(),
    })
}

/// A step of a use case.
pub struct StepView {
    pub order: u32,
    pub actor: String,
    pub human: bool,
    pub action: String,
    pub detail: Option<String>,
    /// The cited lines, `path:start-end`.
    pub cited: Vec<String>,
}

impl StepView {
    fn of(step: &Step) -> Self {
        Self {
            order: step.order,
            actor: step.actor.name.clone(),
            human: step.actor.kind == ActorKind::Human,
            action: step.label().to_string(),
            detail: step.detail().map(str::to_string),
            cited: step.source_refs.iter().map(source_ref_label).collect(),
        }
    }
}

/// The page of a use case, its sections in the order of its Markdown page.
pub struct UseCasePage {
    pub crumbs: Vec<Link>,
    pub name: String,
    pub description: String,
    pub confidence: Confidence,
    /// `None` until the vocabulary pass scored it.
    pub business_language: Option<Confidence>,
    pub primary_actor: Option<String>,
    /// The names of the entry points that trigger it.
    pub triggered_by: Vec<String>,
    pub stale: Vec<StaleFile>,
    pub narrative: Option<String>,
    /// The confidence pass judged the narrative unsupported by the cited code.
    pub unbacked: bool,
    pub unbacked_rationale: Option<String>,
    pub rules: Vec<RuleView>,
    pub steps: Vec<StepView>,
    pub diagram: Option<String>,
    pub docs: Vec<DocItem>,
}

/// The page of the use case `slug` of the feature `feature` of the domain
/// `domain`, `None` when the run has no such use case there. Reads the files
/// its steps cite to tell whether they changed since the run.
#[must_use]
pub fn use_case_page(
    docs: &GeneratedDocs,
    links: &DocLinks,
    domain: &str,
    feature: &str,
    slug: &str,
) -> Option<UseCasePage> {
    let owner = find_feature(docs, domain, feature)?;
    let use_case = docs.use_cases_of(feature).find(|u| u.slug == slug)?;
    let mut cited: Vec<&str> = Vec::new();
    for source in use_case.steps.iter().flat_map(|s| &s.source_refs) {
        if !cited.contains(&source.path.as_str()) {
            cited.push(&source.path);
        }
    }
    // As the debt report counts them: a narrative partly backed states what
    // the cited lines do not show, mostly true of the code around them.
    let unbacked = use_case
        .narrative_check
        .as_ref()
        .filter(|check| check.value <= 0.0);
    Some(UseCasePage {
        crumbs: crumbs_to(docs, Some(owner), domain),
        name: use_case.name.clone(),
        description: use_case.description.trim().to_string(),
        confidence: Confidence::of(use_case.confidence.as_ref()),
        business_language: use_case
            .business_language
            .as_ref()
            .map(|s| Confidence::of(Some(s))),
        primary_actor: use_case.primary_actor.clone(),
        triggered_by: use_case
            .entry_point_names()
            .into_iter()
            .map(str::to_string)
            .collect(),
        stale: stale_files(docs.freshness(), cited),
        narrative: use_case
            .narrative
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string),
        unbacked: unbacked.is_some(),
        unbacked_rationale: unbacked.and_then(|check| Confidence::of(Some(check)).rationale),
        rules: docs
            .rules()
            .iter()
            .filter(|r| r.belongs_to_feature(feature) && r.use_cases.contains(&use_case.slug))
            .map(|r| rule_view(r, owner, &[], links))
            .collect(),
        steps: use_case.steps.iter().map(StepView::of).collect(),
        diagram: use_case
            .diagram_mermaid
            .as_deref()
            .map(str::trim_end)
            .map(str::to_string),
        docs: doc_items(
            links,
            &ItemRef::UseCase {
                feature: feature.to_string(),
                use_case: slug.to_string(),
            },
        ),
    })
}
