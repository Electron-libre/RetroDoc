//! What the agent can find: one [`Entry`] per domain, sub-domain, feature,
//! use case, glossary concept and collected Markdown doc, flattened to text
//! with the identity, confidence and cited files the answers will carry.

use retrodoc_core::model::{Domain, Feature, UseCase};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::Glossary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Domain,
    Feature,
    UseCase,
    Glossary,
    Doc,
}

impl EntryKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            EntryKind::Domain => "domain",
            EntryKind::Feature => "feature",
            EntryKind::UseCase => "use case",
            EntryKind::Glossary => "glossary",
            EntryKind::Doc => "doc",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub kind: EntryKind,
    /// Stable address: `billing`, `billing/payment` (sub-domain),
    /// `billing/pay-invoice` (feature), `billing/pay-invoice/pay-by-card`
    /// (use case), an entity name, or a repo-relative doc path. A feature in
    /// a sub-domain is `billing/payment/pay-invoice`.
    pub id: String,
    pub title: String,
    /// The searchable text.
    pub text: String,
    /// `None` for what the confidence pass doesn't score (glossary, docs) or
    /// hasn't scored yet.
    pub confidence: Option<f32>,
    /// Repo-relative files the entry cites.
    pub sources: Vec<String>,
}

fn lines(parts: impl IntoIterator<Item = String>) -> String {
    parts
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn feature_id(feature: &Feature) -> String {
    match &feature.sub_domain_slug {
        Some(sub) => format!("{}/{sub}/{}", feature.domain_slug, feature.slug),
        None => format!("{}/{}", feature.domain_slug, feature.slug),
    }
}

fn domain_entries(domains: &[Domain]) -> Vec<Entry> {
    let mut entries = Vec::new();
    for domain in domains {
        entries.push(Entry {
            kind: EntryKind::Domain,
            id: domain.slug.clone(),
            title: domain.name.clone(),
            text: lines([domain.name.clone(), domain.description.clone()]),
            confidence: domain.confidence.as_ref().map(|c| c.value),
            sources: Vec::new(),
        });
        for sub in &domain.sub_domains {
            entries.push(Entry {
                kind: EntryKind::Domain,
                id: format!("{}/{}", domain.slug, sub.slug),
                title: sub.name.clone(),
                text: lines([sub.name.clone(), sub.description.clone()]),
                confidence: sub.confidence.as_ref().map(|c| c.value),
                sources: Vec::new(),
            });
        }
    }
    entries
}

fn use_case_entry(use_case: &UseCase, feature_id: &str) -> Entry {
    let mut sources: Vec<String> = Vec::new();
    for step in &use_case.steps {
        for source in &step.source_refs {
            if !sources.contains(&source.path) {
                sources.push(source.path.clone());
            }
        }
    }
    let steps = use_case
        .steps
        .iter()
        .flat_map(|s| [s.description.clone(), s.action.clone()]);
    let text = lines(
        [
            use_case.name.clone(),
            use_case.description.clone(),
            use_case.narrative.clone().unwrap_or_default(),
            use_case.primary_actor.clone().unwrap_or_default(),
            use_case.entry_points.join("\n"),
        ]
        .into_iter()
        .chain(steps),
    );
    Entry {
        kind: EntryKind::UseCase,
        id: format!("{feature_id}/{}", use_case.slug),
        title: use_case.name.clone(),
        text,
        confidence: use_case.confidence.as_ref().map(|c| c.value),
        sources,
    }
}

/// An ATX heading: 1 to 6 `#` then a space (not `#!/bin/sh` or `#hashtag`).
fn is_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes) && line[hashes..].starts_with(' ')
}

fn doc_entry(doc: &ExistingDoc) -> Entry {
    let path = doc.path.to_string_lossy().replace('\\', "/");
    let title = doc.content.lines().find(|l| is_heading(l)).map_or_else(
        || path.clone(),
        |t| t.trim_start_matches('#').trim().to_string(),
    );
    Entry {
        kind: EntryKind::Doc,
        id: path.clone(),
        text: lines([title.clone(), doc.content.clone()]),
        title,
        confidence: None,
        sources: vec![path],
    }
}

/// Every searchable entry, in a stable order: domains, features, use cases,
/// glossary, docs.
#[must_use]
pub fn build_entries(
    domains: &[Domain],
    features: &[Feature],
    use_cases: &[UseCase],
    glossary: &Glossary,
    docs: &[ExistingDoc],
) -> Vec<Entry> {
    let mut entries = domain_entries(domains);
    for feature in features {
        entries.push(Entry {
            kind: EntryKind::Feature,
            id: feature_id(feature),
            title: feature.name.clone(),
            text: lines([feature.name.clone(), feature.description.clone()]),
            confidence: feature.confidence.as_ref().map(|c| c.value),
            sources: feature.source_paths.clone(),
        });
    }
    for use_case in use_cases {
        let parent = features
            .iter()
            .find(|f| f.slug == use_case.feature_slug)
            .map_or_else(|| use_case.feature_slug.clone(), feature_id);
        entries.push(use_case_entry(use_case, &parent));
    }
    for entity in glossary.merged_entities() {
        let associations = entity
            .associations
            .iter()
            .map(|a| format!("{} {}", a.kind, a.target));
        entries.push(Entry {
            kind: EntryKind::Glossary,
            id: entity.name.clone(),
            title: entity.name.clone(),
            text: lines(
                [entity.name.clone(), entity.description.clone()]
                    .into_iter()
                    .chain(entity.attributes.iter().cloned())
                    .chain(associations),
            ),
            confidence: None,
            sources: entity
                .files
                .iter()
                .map(|f| f.to_string_lossy().into_owned())
                .collect(),
        });
    }
    entries.extend(docs.iter().map(doc_entry));
    entries
}

#[cfg(test)]
pub(crate) mod fixtures {
    use retrodoc_core::model::{
        Actor, ActorKind, ConfidenceScore, Domain, Feature, SourceRef, Step, SubDomain, UseCase,
    };

    pub fn domain(slug: &str, name: &str, description: &str) -> Domain {
        Domain {
            slug: slug.into(),
            name: name.into(),
            description: description.into(),
            sub_domains: vec![SubDomain {
                slug: "payment".into(),
                name: "Payment".into(),
                description: "Collecting the money".into(),
                confidence: None,
            }],
            confidence: Some(ConfidenceScore::new(0.8, None)),
        }
    }

    pub fn feature(slug: &str, name: &str, description: &str) -> Feature {
        Feature {
            slug: slug.into(),
            domain_slug: "billing".into(),
            sub_domain_slug: None,
            name: name.into(),
            description: description.into(),
            source_paths: vec!["app/invoice.rb".into()],
            confidence: None,
        }
    }

    pub fn use_case(slug: &str, feature: &str, name: &str, narrative: &str) -> UseCase {
        let step = |order, description: &str, path: &str| Step {
            order,
            description: description.into(),
            actor: Actor {
                name: "Accountant".into(),
                kind: ActorKind::Human,
            },
            action: "records".into(),
            source_refs: vec![SourceRef {
                path: path.into(),
                start_line: None,
                end_line: None,
            }],
        };
        UseCase {
            slug: slug.into(),
            feature_slug: feature.into(),
            name: name.into(),
            description: String::new(),
            steps: vec![
                step(1, "Checks the amount", "app/invoice.rb"),
                step(2, "Marks it paid", "app/invoice.rb"),
                step(3, "Notifies the customer", "app/mailer.rb"),
            ],
            entry_points: vec!["POST /invoices/:id/pay".into()],
            primary_actor: Some("Accountant".into()),
            narrative: Some(narrative.into()),
            business_language: None,
            diagram_mermaid: None,
            confidence: Some(ConfidenceScore::new(0.9, None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::fixtures::*;
    use super::*;

    fn entries() -> Vec<Entry> {
        let docs = [ExistingDoc {
            path: PathBuf::from("docs/billing.md"),
            content: "Intro\n\n## Late fees\nCharged after 30 days.".into(),
        }];
        build_entries(
            &[domain("billing", "Billing", "Invoices and payments")],
            &[feature(
                "pay-invoice",
                "Pay an invoice",
                "Settle what is due",
            )],
            &[use_case(
                "pay-by-card",
                "pay-invoice",
                "Pay by card",
                "The accountant settles the invoice",
            )],
            &Glossary::default(),
            &docs,
        )
    }

    #[test]
    fn each_level_gets_an_addressable_entry() {
        let all: Vec<String> = entries().into_iter().map(|e| e.id).collect();
        assert_eq!(
            all,
            [
                "billing",
                "billing/payment",
                "billing/pay-invoice",
                "billing/pay-invoice/pay-by-card",
                "docs/billing.md"
            ]
        );
    }

    #[test]
    fn a_use_case_carries_its_text_confidence_and_distinct_cited_files() {
        let entries = entries();
        let use_case = entries
            .iter()
            .find(|e| e.kind == EntryKind::UseCase)
            .unwrap();
        assert!(use_case.text.contains("The accountant settles the invoice"));
        assert!(use_case.text.contains("Marks it paid"));
        assert!(use_case.text.contains("POST /invoices/:id/pay"));
        assert_eq!(use_case.confidence, Some(0.9));
        assert_eq!(use_case.sources, ["app/invoice.rb", "app/mailer.rb"]);
    }

    #[test]
    fn a_feature_in_a_sub_domain_is_addressed_through_it() {
        let mut feature = feature("refund", "Refund", "Give money back");
        feature.sub_domain_slug = Some("payment".into());
        let entries = build_entries(&[], &[feature], &[], &Glossary::default(), &[]);
        assert_eq!(entries[0].id, "billing/payment/refund");
    }

    #[test]
    fn a_doc_is_titled_by_its_first_heading_or_else_its_path() {
        let entries = entries();
        let doc = entries.last().unwrap();
        assert_eq!(doc.title, "Late fees");
        assert_eq!(doc.sources, ["docs/billing.md"]);
        let shebang = doc_entry(&ExistingDoc {
            path: PathBuf::from("run.md"),
            content: "#!/bin/sh\n#tag\n# Real title".into(),
        });
        assert_eq!(shebang.title, "Real title");
        let plain = doc_entry(&ExistingDoc {
            path: PathBuf::from("notes.md"),
            content: "no heading".into(),
        });
        assert_eq!(plain.title, "notes.md");
    }
}
