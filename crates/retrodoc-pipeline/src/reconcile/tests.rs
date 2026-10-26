use std::path::PathBuf;

use retrodoc_core::model::{ConfidenceScore, RuleKind};

use super::*;
use crate::domains::DomainCluster;

fn doc(path: &str, content: &str) -> ExistingDoc {
    ExistingDoc {
        path: PathBuf::from(path),
        content: content.to_string(),
    }
}

fn domain(slug: &str, name: &str) -> DomainCluster {
    DomainCluster {
        slug: slug.to_string(),
        name: name.to_string(),
        ..DomainCluster::default()
    }
}

fn feature(slug: &str, domain: &str, name: &str, entry_points: usize) -> Feature {
    Feature {
        back_office: false,
        slug: slug.to_string(),
        domain_slug: domain.to_string(),
        sub_domain_slug: None,
        name: name.to_string(),
        description: String::new(),
        source_paths: vec![],
        entry_points: (0..entry_points).map(|i| format!("a.rb#e{i}")).collect(),
        confidence: None,
    }
}

fn use_case(slug: &str, feature: &str, name: &str) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.to_string(),
        feature_slug: feature.to_string(),
        name: name.to_string(),
        description: String::new(),
        steps: vec![],
        diagram_mermaid: None,
        confidence: None,
    }
}

fn rule(slug: &str, statement: &str, evidence: Vec<RuleEvidence>) -> BusinessRule {
    BusinessRule {
        slug: slug.to_string(),
        statement: statement.to_string(),
        kind: RuleKind::Limit,
        entities: Vec::new(),
        use_cases: Vec::new(),
        evidence,
        confidence: None,
    }
}

fn verdict(label: &str, entry_points: usize, confidence: Option<f32>) -> ItemVerdict {
    ItemVerdict {
        kind: ItemKind::Feature,
        label: label.to_string(),
        domain: "d".to_string(),
        name: label.to_string(),
        entry_points,
        confidence,
        status: Status::Undocumented,
        sections: Vec::new(),
    }
}

// spec: REC-2
#[test]
fn an_item_keeps_at_most_three_sections_sharing_half_of_its_words_best_first() {
    let docs = [doc(
        "docs/guide.md",
        "# Invoices\nPay an invoice by card.\n\n# Paying\nPay the invoice.\n\n# Cards\nInvoice card.\n\n\
         # Again\nPay an invoice, pay it, the invoice.\n\n# Riders\nRiders deliver orders.\n",
    )];
    let corpus = corpus(&docs);
    let found = candidates(
        vec![
            Item {
                query: "Pay invoice".into(),
                ..items(
                    &DomainMap::default(),
                    &[feature("pay", "billing", "x", 0)],
                    &[],
                    &[],
                )
                .remove(0)
            },
            Item {
                query: "It is what it is".into(),
                ..items(
                    &DomainMap::default(),
                    &[feature("it", "billing", "x", 0)],
                    &[],
                    &[],
                )
                .remove(0)
            },
        ],
        &corpus,
    );

    assert_eq!(found.len(), 1, "a query of stop words only is left out");
    let origins: Vec<String> = found[0].sections.iter().map(|s| s.at.origin()).collect();
    assert_eq!(origins.len(), MAX_SECTIONS_PER_ITEM);
    assert_eq!(origins[0], "docs/guide.md#Again");
    assert!(!origins.contains(&"docs/guide.md#Riders".to_string()));
}

// spec: REC-2
#[test]
fn every_kind_of_item_is_searched_with_its_name_and_description() {
    let map = DomainMap {
        domains: vec![
            DomainCluster {
                description: "Money owed".into(),
                ..domain("billing", "Billing")
            },
            domain(UNCATEGORIZED_SLUG, "Uncategorized"),
        ],
    };
    let mut pay = feature("pay", "billing", "Payment", 2);
    pay.description = "Settle a bill".into();
    let mut by_card = use_case("by-card", "pay", "Pay by card");
    by_card.description = "With a card".into();
    let found = items(
        &map,
        &[pay, feature("refund", "billing", "Refunds", 1)],
        &[by_card],
        &[rule("pay-1", "A bill is paid once.", vec![])],
    );

    let labels: Vec<(&str, ItemKind, &str, usize)> = found
        .iter()
        .map(|i| (i.label.as_str(), i.kind, i.query.as_str(), i.entry_points))
        .collect();
    assert_eq!(
        labels,
        [
            ("billing", ItemKind::Domain, "Billing\nMoney owed", 3),
            (
                "billing/pay",
                ItemKind::Feature,
                "Payment\nSettle a bill",
                2
            ),
            ("billing/refund", ItemKind::Feature, "Refunds\n", 1),
            (
                "billing/pay/by-card",
                ItemKind::UseCase,
                "Pay by card\nWith a card",
                0
            ),
            (
                "billing/pay/pay-1",
                ItemKind::Rule,
                "A bill is paid once.",
                2
            ),
        ]
    );
}

// spec: REC-3
#[test]
fn no_matching_section_is_undocumented_and_a_cited_doc_section_documents_a_rule() {
    let docs = [doc("README.md", "# Limits\nA cart holds ten items.\n")];
    let rules = [
        rule("cart-1", "Riders above the weekly cap are paused.", vec![]),
        rule(
            "cart-2",
            "Cooking time counts before the rider leaves.",
            vec![RuleEvidence::Doc {
                path: "README.md".into(),
                heading: "Limits".into(),
            }],
        ),
        rule("cart-3", "A cart holds ten items at most.", vec![]),
    ];

    let result = reconcile(&DomainMap::default(), &[], &[], &rules, &docs);
    let status: Vec<(&str, Status, Vec<String>)> = result
        .items
        .iter()
        .map(|i| {
            (
                i.label.as_str(),
                i.status,
                i.sections.iter().map(SectionRef::origin).collect(),
            )
        })
        .collect();
    assert_eq!(
        status,
        [
            ("?/?/cart-1", Status::Undocumented, vec![]),
            (
                "?/?/cart-2",
                Status::Documented,
                vec!["README.md#Limits".to_string()]
            ),
            (
                "?/?/cart-3",
                Status::Matched,
                vec!["README.md#Limits".to_string()]
            ),
        ]
    );
}

// spec: REC-10
#[test]
fn undocumented_items_rank_by_entry_points_then_confidence_then_label() {
    let mut matched = verdict("m", 9, None);
    matched.status = Status::Matched;
    let reconciliation = Reconciliation {
        items: vec![
            verdict("b-unscored", 1, None),
            verdict("c-low", 1, Some(0.2)),
            verdict("a-high", 1, Some(0.9)),
            verdict("z-many", 4, None),
            verdict("a-unscored", 1, None),
            matched,
        ],
    };

    let order: Vec<&str> = reconciliation
        .undocumented()
        .iter()
        .map(|i| i.label.as_str())
        .collect();
    assert_eq!(
        order,
        ["z-many", "a-high", "c-low", "a-unscored", "b-unscored"]
    );
}

// spec: REC-9
#[test]
fn each_domain_counts_its_items_per_status() {
    let mut documented = verdict("x", 0, None);
    documented.status = Status::Documented;
    let mut other = verdict("y", 0, None);
    other.domain = "other".into();
    let reconciliation = Reconciliation {
        items: vec![documented, verdict("z", 0, None), other],
    };

    let counts = reconciliation.by_domain();
    assert_eq!(
        counts["d"],
        StatusCount {
            documented: 1,
            undocumented: 1,
            ..StatusCount::default()
        }
    );
    assert_eq!(counts["other"].total(), 1);
}

#[test]
fn a_domain_is_as_trusted_as_its_features() {
    let mut a = feature("a", "billing", "A", 0);
    a.confidence = Some(ConfidenceScore::new(0.4, None));
    let mut b = feature("b", "billing", "B", 0);
    b.confidence = Some(ConfidenceScore::new(0.8, None));
    let map = DomainMap {
        domains: vec![domain("billing", "Billing")],
    };
    let found = items(&map, &[a, b], &[], &[]);
    assert!((found[0].confidence.unwrap() - 0.6).abs() < 1e-6);
}
