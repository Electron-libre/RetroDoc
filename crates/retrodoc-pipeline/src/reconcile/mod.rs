//! Reconciliation of the generated docs with the project's own documents (`specs/docs-reconciliation.md`,
//! ADR 0033). Every domain, feature, use case and business rule is matched with BM25 against the sections
//! of the existing docs; an item no section matches is undocumented with no LLM call. What a matched item
//! is (documented, contradicted, or a false match) is the LLM's answer, until then it is only matched.
//! The debt report and the rendered pages read the result; neither calls an LLM.

use std::collections::BTreeMap;

use retrodoc_core::model::{BusinessRule, Feature, RuleEvidence, UseCase};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_ingest::signals::doc_sections;
use serde::{Deserialize, Serialize};

use crate::bm25::{has_content_words, Bm25};
use crate::domains::{DomainMap, UNCATEGORIZED_SLUG};

/// Sections kept per item, best first.
pub const MAX_SECTIONS_PER_ITEM: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    Domain,
    Feature,
    UseCase,
    Rule,
}

impl ItemKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Feature => "feature",
            Self::UseCase => "use case",
            Self::Rule => "rule",
        }
    }
}

/// A generated item to reconcile.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    /// `<domain>`, `<domain>/<feature>`, `<domain>/<feature>/<use case>` or `<domain>/<feature>/<rule>`
    /// (`?` for a feature or domain that no longer exists).
    pub label: String,
    pub domain: String,
    /// The name, or the statement of a rule.
    pub name: String,
    /// What the corpus is searched with: the name and the description.
    pub query: String,
    /// Entry points the item holds (a domain: those of its features; a rule: those of its feature).
    pub entry_points: usize,
    pub confidence: Option<f32>,
    /// Doc sections the item cites itself (`path#heading`): the evidence of a rule.
    pub cited_sections: Vec<String>,
}

/// Where a doc section is: its file and its heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionRef {
    pub path: String,
    /// `None` for the text before the first heading.
    pub heading: Option<String>,
}

impl SectionRef {
    /// Reads `path#heading`, or `path` alone.
    #[must_use]
    pub fn parse(origin: &str) -> Self {
        match origin.split_once('#') {
            Some((path, heading)) => Self {
                path: path.to_string(),
                heading: Some(heading.to_string()),
            },
            None => Self {
                path: origin.to_string(),
                heading: None,
            },
        }
    }

    /// `path#heading`, or `path` alone.
    #[must_use]
    pub fn origin(&self) -> String {
        match &self.heading {
            Some(h) => format!("{}#{h}", self.path),
            None => self.path.clone(),
        }
    }
}

/// A section of an existing doc: one heading and its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub at: SectionRef,
    /// The heading line followed by the body.
    pub text: String,
}

/// The sections of `docs`, which must not hold the generated docs (the caller drops them).
#[must_use]
pub fn corpus(docs: &[ExistingDoc]) -> Vec<Section> {
    doc_sections(docs)
        .into_iter()
        .map(|s| Section {
            at: SectionRef::parse(&s.origin),
            text: s.text,
        })
        .collect()
}

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0_f32, 0_u32), |(s, n), v| (s + v, n + 1));
    #[allow(clippy::cast_precision_loss)]
    (n > 0).then(|| sum / n as f32)
}

/// The items of the saved artifacts, in the order domains, features, use cases, rules; the
/// "uncategorized" bucket is not a domain.
#[must_use]
pub fn items(
    domains: &DomainMap,
    features: &[Feature],
    use_cases: &[UseCase],
    rules: &[BusinessRule],
) -> Vec<Item> {
    let by_slug: BTreeMap<&str, &Feature> = features.iter().map(|f| (f.slug.as_str(), f)).collect();
    let mut items = Vec::new();
    for domain in domains
        .domains
        .iter()
        .filter(|d| d.slug != UNCATEGORIZED_SLUG)
    {
        let own = || features.iter().filter(|f| f.domain_slug == domain.slug);
        items.push(Item {
            kind: ItemKind::Domain,
            label: domain.slug.clone(),
            domain: domain.slug.clone(),
            name: domain.name.clone(),
            query: format!("{}\n{}", domain.name, domain.description),
            entry_points: own().map(|f| f.entry_points.len()).sum(),
            confidence: mean(own().filter_map(|f| f.confidence.as_ref().map(|c| c.value))),
            cited_sections: Vec::new(),
        });
    }
    for feature in features {
        items.push(Item {
            kind: ItemKind::Feature,
            label: format!("{}/{}", feature.domain_slug, feature.slug),
            domain: feature.domain_slug.clone(),
            name: feature.name.clone(),
            query: format!("{}\n{}", feature.name, feature.description),
            entry_points: feature.entry_points.len(),
            confidence: feature.confidence.as_ref().map(|c| c.value),
            cited_sections: Vec::new(),
        });
    }
    for use_case in use_cases {
        let domain = by_slug
            .get(use_case.feature_slug.as_str())
            .map_or("?", |f| f.domain_slug.as_str());
        items.push(Item {
            kind: ItemKind::UseCase,
            label: format!("{domain}/{}/{}", use_case.feature_slug, use_case.slug),
            domain: domain.to_string(),
            name: use_case.name.clone(),
            query: format!("{}\n{}", use_case.name, use_case.description),
            entry_points: use_case.entry_points.len(),
            confidence: use_case.confidence.as_ref().map(|c| c.value),
            cited_sections: Vec::new(),
        });
    }
    for rule in rules {
        let feature = features.iter().find(|f| rule.belongs_to_feature(&f.slug));
        let (domain, feature_slug) =
            feature.map_or(("?", "?"), |f| (f.domain_slug.as_str(), f.slug.as_str()));
        items.push(Item {
            kind: ItemKind::Rule,
            label: format!("{domain}/{feature_slug}/{}", rule.slug),
            domain: domain.to_string(),
            name: rule.statement.clone(),
            query: rule.statement.clone(),
            entry_points: feature.map_or(0, |f| f.entry_points.len()),
            confidence: rule.confidence.as_ref().map(|c| c.value),
            cited_sections: rule
                .evidence
                .iter()
                .filter_map(|e| match e {
                    RuleEvidence::Doc { path, heading } if heading.is_empty() => Some(path.clone()),
                    RuleEvidence::Doc { path, heading } => Some(format!("{path}#{heading}")),
                    _ => None,
                })
                .collect(),
        });
    }
    items
}

/// An item and the sections it was matched to.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub item: Item,
    /// Best first, at most [`MAX_SECTIONS_PER_ITEM`]. For an item that cites doc sections itself, those
    /// sections (from the corpus when they are in it).
    pub sections: Vec<Section>,
}

impl Candidate {
    /// Whether the item cites the doc sections it rests on, so needs no judgement.
    #[must_use]
    pub fn cites_its_docs(&self) -> bool {
        !self.item.cited_sections.is_empty()
    }
}

/// Matches each item against `corpus`; an item whose query has no content word is left out.
#[must_use]
pub fn candidates(items: Vec<Item>, corpus: &[Section]) -> Vec<Candidate> {
    // The path is not searched: a file named after a topic does not say what it states.
    let texts: Vec<String> = corpus.iter().map(|s| s.text.clone()).collect();
    let index = Bm25::new(&texts);
    items
        .into_iter()
        .filter_map(|item| {
            if !item.cited_sections.is_empty() {
                let sections = item
                    .cited_sections
                    .iter()
                    .map(|origin| {
                        corpus
                            .iter()
                            .find(|s| s.at.origin() == *origin)
                            .cloned()
                            .unwrap_or_else(|| Section {
                                at: SectionRef::parse(origin),
                                text: String::new(),
                            })
                    })
                    .collect();
                return Some(Candidate { item, sections });
            }
            if !has_content_words(&item.query) {
                return None;
            }
            let sections = index
                .search(&item.query)
                .into_iter()
                .take(MAX_SECTIONS_PER_ITEM)
                .map(|(i, _)| corpus[i].clone())
                .collect();
            Some(Candidate { item, sections })
        })
        .collect()
}

/// Where an item stands against the existing docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// A section covers it (judged, or cited by the item itself).
    Documented,
    /// Sections match it but no judgement says whether they cover it.
    Matched,
    Undocumented,
    /// A section states something else than the generated item.
    Contradicted,
}

/// The verdict on one item.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemVerdict {
    pub kind: ItemKind,
    pub label: String,
    pub domain: String,
    pub name: String,
    pub entry_points: usize,
    pub confidence: Option<f32>,
    pub status: Status,
    /// The sections that document or contradict it, best first (the matched ones while not judged).
    pub sections: Vec<SectionRef>,
}

/// The verdicts on every item.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reconciliation {
    pub items: Vec<ItemVerdict>,
}

impl Reconciliation {
    /// The verdicts that need no LLM: no section is undocumented, sections the item cites itself
    /// document it, other sections only match it.
    #[must_use]
    pub fn from_candidates(candidates: Vec<Candidate>) -> Self {
        let items = candidates
            .into_iter()
            .map(|c| {
                let status = if c.sections.is_empty() {
                    Status::Undocumented
                } else if c.cites_its_docs() {
                    Status::Documented
                } else {
                    Status::Matched
                };
                ItemVerdict {
                    kind: c.item.kind,
                    label: c.item.label,
                    domain: c.item.domain,
                    name: c.item.name,
                    entry_points: c.item.entry_points,
                    confidence: c.item.confidence,
                    status,
                    sections: c.sections.into_iter().map(|s| s.at).collect(),
                }
            })
            .collect();
        Self { items }
    }

    /// The undocumented items, most entry points first, then most trusted (unscored last), then by
    /// label.
    #[must_use]
    pub fn undocumented(&self) -> Vec<&ItemVerdict> {
        let mut items: Vec<&ItemVerdict> = self
            .items
            .iter()
            .filter(|i| i.status == Status::Undocumented)
            .collect();
        items.sort_by(|a, b| {
            b.entry_points
                .cmp(&a.entry_points)
                .then_with(|| match (a.confidence, b.confidence) {
                    (Some(x), Some(y)) => y.total_cmp(&x),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                })
                .then_with(|| a.label.cmp(&b.label))
        });
        items
    }

    /// Per domain, how many items have each status.
    #[must_use]
    pub fn by_domain(&self) -> BTreeMap<&str, StatusCount> {
        let mut counts: BTreeMap<&str, StatusCount> = BTreeMap::new();
        for item in &self.items {
            let count = counts.entry(item.domain.as_str()).or_default();
            match item.status {
                Status::Documented => count.documented += 1,
                Status::Matched => count.matched += 1,
                Status::Undocumented => count.undocumented += 1,
                Status::Contradicted => count.contradicted += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusCount {
    pub documented: usize,
    pub matched: usize,
    pub undocumented: usize,
    pub contradicted: usize,
}

impl StatusCount {
    #[must_use]
    pub fn total(&self) -> usize {
        self.documented + self.matched + self.undocumented + self.contradicted
    }
}

/// The verdicts that need no LLM on the saved artifacts and the project's own docs.
#[must_use]
pub fn reconcile(
    domains: &DomainMap,
    features: &[Feature],
    use_cases: &[UseCase],
    rules: &[BusinessRule],
    docs: &[ExistingDoc],
) -> Reconciliation {
    Reconciliation::from_candidates(candidates(
        items(domains, features, use_cases, rules),
        &corpus(docs),
    ))
}

#[cfg(test)]
mod tests;
