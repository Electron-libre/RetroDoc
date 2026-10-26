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

mod judge;

pub use judge::{judge, Answer, Judged, JudgedSection, Judgements, MAX_ITEMS_PER_REQUEST};

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

/// The sections of `docs`, which must not hold the generated docs (the caller drops them). Sections
/// of one file under the same heading are one section: an origin names one text.
#[must_use]
pub fn corpus(docs: &[ExistingDoc]) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for signal in doc_sections(docs) {
        let at = SectionRef::parse(&signal.origin);
        match sections.iter_mut().find(|s| s.at == at) {
            Some(same) => {
                same.text.push_str("\n\n");
                same.text.push_str(&signal.text);
            }
            None => sections.push(Section {
                at,
                text: signal.text,
            }),
        }
    }
    sections
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

/// Matches each item against `corpus`, with its name alone when its query matches nothing; an item whose
/// query has no content word is left out.
#[must_use]
pub fn candidates(items: Vec<Item>, corpus: &[Section]) -> Vec<Candidate> {
    // The path is not searched: a file named after a topic does not say what it states.
    let texts: Vec<String> = corpus.iter().map(|s| s.text.clone()).collect();
    let index = Bm25::new(&texts);
    items
        .into_iter()
        .filter_map(|mut item| {
            // A cited section no longer in the docs backs nothing.
            item.cited_sections
                .retain(|origin| corpus.iter().any(|s| s.at.origin() == *origin));
            if !item.cited_sections.is_empty() {
                let sections = corpus
                    .iter()
                    .filter(|s| item.cited_sections.contains(&s.at.origin()))
                    .cloned()
                    .collect();
                return Some(Candidate { item, sections });
            }
            if !has_content_words(&item.query) {
                return None;
            }
            // A description of two sentences rarely shares half its words with the section that
            // documents it in other words; a short name often does.
            let mut found = index.search(&item.query);
            if found.is_empty() {
                found = index.search(&item.name);
            }
            let sections = found
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
    /// What each contradicting section states against the item.
    pub contradictions: Vec<Contradiction>,
}

/// A section that states something incompatible with the generated item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contradiction {
    pub section: SectionRef,
    pub doc_states: String,
    pub item_states: String,
}

/// The verdicts on every item.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reconciliation {
    pub items: Vec<ItemVerdict>,
}

impl Reconciliation {
    /// The verdicts on `candidates`: no section is undocumented, sections the item cites itself
    /// document it; otherwise the saved judgement while the texts are unchanged decides (one
    /// contradicting section makes it contradicted, else one covering section documented, else an
    /// unanswered section leaves it matched, else undocumented), and with none it is matched.
    #[must_use]
    pub fn new(candidates: Vec<Candidate>, judgements: &Judgements) -> Self {
        let items = candidates
            .into_iter()
            .map(|c| {
                let (status, sections, contradictions) = if c.sections.is_empty() {
                    (Status::Undocumented, Vec::new(), Vec::new())
                } else if c.cites_its_docs() {
                    (Status::Documented, at(c.sections), Vec::new())
                } else if let Some(judged) = judgements.of(&c) {
                    verdict_of(judged)
                } else {
                    (Status::Matched, at(c.sections), Vec::new())
                };
                ItemVerdict {
                    kind: c.item.kind,
                    label: c.item.label,
                    domain: c.item.domain,
                    name: c.item.name,
                    entry_points: c.item.entry_points,
                    confidence: c.item.confidence,
                    status,
                    sections,
                    contradictions,
                }
            })
            .collect();
        Self { items }
    }

    /// The contradicted items, in the order of the items.
    #[must_use]
    pub fn contradicted(&self) -> Vec<&ItemVerdict> {
        self.items
            .iter()
            .filter(|i| i.status == Status::Contradicted)
            .collect()
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

fn at(sections: Vec<Section>) -> Vec<SectionRef> {
    sections.into_iter().map(|s| s.at).collect()
}

/// Status, sections behind it and contradictions of a judged item.
fn verdict_of(judged: &Judged) -> (Status, Vec<SectionRef>, Vec<Contradiction>) {
    let with = |answer: Answer| -> Vec<&JudgedSection> {
        judged
            .sections
            .iter()
            .filter(|s| s.answer == answer)
            .collect()
    };
    let (covering, contradicting) = (with(Answer::Covers), with(Answer::Contradicts));
    let contradictions: Vec<Contradiction> = contradicting
        .iter()
        .filter_map(|s| {
            Some(Contradiction {
                section: SectionRef::parse(&s.section),
                doc_states: s.doc_states.clone()?,
                item_states: s.item_states.clone()?,
            })
        })
        .collect();
    let status = if !contradictions.is_empty() {
        Status::Contradicted
    } else if !covering.is_empty() {
        Status::Documented
    } else {
        Status::Undocumented
    };
    let sections = judged
        .sections
        .iter()
        .filter(|s| s.answer != Answer::Unrelated)
        .map(|s| SectionRef::parse(&s.section))
        .collect();
    (status, sections, contradictions)
}

/// The verdicts on the saved artifacts and the project's own docs, with the saved `judgements`.
#[must_use]
pub fn reconcile(
    domains: &DomainMap,
    features: &[Feature],
    use_cases: &[UseCase],
    rules: &[BusinessRule],
    docs: &[ExistingDoc],
    judgements: &Judgements,
) -> Reconciliation {
    Reconciliation::new(
        candidates(items(domains, features, use_cases, rules), &corpus(docs)),
        judgements,
    )
}

#[cfg(test)]
mod tests;
