use std::path::PathBuf;

use retrodoc_core::model::{BusinessRule, ConfidenceScore, SourceRef};

use super::super::{render, render_with_docs, render_with_rules};
use super::{meta, page, sample};

fn rules() -> Vec<BusinessRule> {
    use retrodoc_core::model::{RuleEvidence, RuleKind};
    vec![
        BusinessRule {
            slug: "pay-invoice-1".to_string(),
            statement: "An invoice already paid\ncannot be paid again.".to_string(),
            kind: RuleKind::StateTransition,
            entities: vec!["Invoice".to_string()],
            use_cases: vec!["pay-by-card".to_string()],
            evidence: vec![
                RuleEvidence::Code {
                    source: SourceRef {
                        path: "src/pay.rs".to_string(),
                        start_line: Some(3),
                        end_line: Some(9),
                    },
                },
                RuleEvidence::Test {
                    phrase: "refuses a second payment".to_string(),
                    path: "spec/pay_spec.rb".to_string(),
                },
                RuleEvidence::Doc {
                    path: "README.md".to_string(),
                    heading: "Payments".to_string(),
                },
            ],
            confidence: Some(ConfidenceScore::new(0.85, None)),
        },
        BusinessRule {
            slug: "pay-invoice-2".to_string(),
            statement: "A payment above the limit is refused.".to_string(),
            kind: RuleKind::Limit,
            entities: vec!["Invoice".to_string(), "Payment".to_string()],
            use_cases: Vec::new(),
            evidence: vec![RuleEvidence::Code {
                source: SourceRef {
                    path: "src/pay.rs".to_string(),
                    start_line: None,
                    end_line: None,
                },
            }],
            confidence: Some(ConfidenceScore::new(0.4, None)),
        },
        BusinessRule {
            slug: "list-1".to_string(),
            statement: "Only open invoices are listed.".to_string(),
            kind: RuleKind::Validation,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: Vec::new(),
            confidence: None,
        },
    ]
}

#[test]
fn rules_are_on_the_pages_of_their_feature_use_cases_and_domain() {
    let (d, f, u) = sample();
    let files = render_with_rules(&d, &f, &u, &rules(), "r", &meta()).unwrap();
    let content = |p: &str| {
        files
            .iter()
            .find(|f| f.path.display().to_string() == p)
            .unwrap_or_else(|| panic!("{p} missing"))
            .content
            .clone()
    };

    let feature = content("functional/billing/pay-invoice.md");
    assert!(feature.contains("## Business rules\n\n- An invoice already paid cannot be paid again. (State transition, 85%)"), "{feature}");
    assert!(
        feature.contains("  - Use cases: [Pay by card](use-cases/pay-invoice/pay-by-card.md)"),
        "{feature}"
    );
    assert!(feature.contains("  - Code: `src/pay.rs:3-9`"), "{feature}");
    assert!(
        feature.contains("  - Test: \"refuses a second payment\" (`spec/pay_spec.rb`)"),
        "{feature}"
    );
    assert!(
        feature.contains("  - Documentation: `README.md`, section \"Payments\""),
        "{feature}"
    );
    assert!(feature.contains("(Limit, ⚠️ 40%)"), "{feature}");
    assert!(
        feature.contains("  - Entities: Invoice, Payment"),
        "{feature}"
    );
    assert!(!feature.contains("Only open invoices"), "{feature}");

    let use_case = content("functional/billing/use-cases/pay-invoice/pay-by-card.md");
    assert!(use_case.contains("## Business rules"), "{use_case}");
    assert!(use_case.contains("cannot be paid again"), "{use_case}");
    assert!(!use_case.contains("above the limit"), "{use_case}");

    let domain = content("functional/billing/README.md");
    assert!(domain.contains("## Business rules"), "{domain}");
    assert!(domain.contains("- A payment above the limit is refused. (Limit, ⚠️ 40%) — [Pay an invoice](pay-invoice.md)"), "{domain}");
    assert!(
        domain
            .contains("Only open invoices are listed. (Validation, not scored) — [List](list.md)"),
        "{domain}"
    );
}

// spec: REC-12
#[test]
fn the_pages_link_the_doc_sections_that_document_or_contradict_them() {
    use crate::{DocLink, DocLinks, ItemRef};
    let (d, f, u) = sample();
    let link = |path: &str, heading: Option<&str>, contradicts: Option<&str>| DocLink {
        path: path.to_string(),
        heading: heading.map(str::to_string),
        contradicts: contradicts.map(str::to_string),
    };
    let mut docs = DocLinks {
        docs_dir: PathBuf::from("docs"),
        ..DocLinks::default()
    };
    docs.links.insert(
        ItemRef::Domain("billing".into()),
        vec![link("docs/guide.md", Some("Billing"), None)],
    );
    docs.links.insert(
        ItemRef::Feature("pay-invoice".into()),
        vec![link(
            "README.md",
            None,
            Some("Invoices are paid by wire only."),
        )],
    );
    docs.links.insert(
        ItemRef::UseCase {
            feature: "pay-invoice".into(),
            use_case: "pay-by-card".into(),
        },
        vec![link("docs/guide.md", Some("Paying by card!"), None)],
    );
    docs.links.insert(
        ItemRef::Rule("pay-invoice-2".into()),
        vec![link("docs/guide.md", Some("Limits"), None)],
    );
    let files = || render_with_docs(&d, &f, &u, &rules(), &docs, "r", &meta()).unwrap();
    let files = files();

    let domain = page(&files, "functional/billing/README.md");
    assert!(
        domain.contains(
            "## Existing documentation\n\n- [`docs/guide.md`, section \"Billing\"](../../guide.md#billing)\n"
        ),
        "{domain}"
    );
    let feature = page(&files, "functional/billing/pay-invoice.md");
    assert!(
        feature.contains(
            "- [`README.md`](../../../README.md) ⚠️ states otherwise: Invoices are paid by wire only.\n"
        ),
        "{feature}"
    );
    assert!(
        feature.contains(
            "  - Existing doc: [`docs/guide.md`, section \"Limits\"](../../guide.md#limits)\n"
        ),
        "{feature}"
    );
    let use_case = page(
        &files,
        "functional/billing/use-cases/pay-invoice/pay-by-card.md",
    );
    assert!(
        use_case.contains("](../../../../guide.md#paying-by-card)"),
        "{use_case}"
    );
    assert!(!page(&files, "functional/billing/list.md").contains("Existing documentation"));
    assert_eq!(
        files,
        render_with_docs(&d, &f, &u, &rules(), &docs, "r", &meta()).unwrap()
    );
    assert_eq!(
        render_with_rules(&d, &f, &u, &rules(), "r", &meta()).unwrap(),
        render_with_docs(&d, &f, &u, &rules(), &DocLinks::default(), "r", &meta()).unwrap()
    );
}

#[test]
fn the_rules_index_groups_by_entity_and_the_agent_index_points_to_it() {
    let (d, f, u) = sample();
    let files = render_with_rules(&d, &f, &u, &rules(), "r", &meta()).unwrap();
    let content = |p: &str| {
        files
            .iter()
            .find(|f| f.path.display().to_string() == p)
            .unwrap()
            .content
            .clone()
    };
    let index = content("functional/rules.md");
    let invoice = index.find("## Invoice").unwrap();
    let payment = index.find("## Payment").unwrap();
    let other = index.find("## Other rules").unwrap();
    assert!(invoice < payment && payment < other, "{index}");
    assert!(index.contains("3 rule(s)"), "{index}");
    assert!(
        index.contains("— [Pay an invoice](billing/pay-invoice.md)"),
        "{index}"
    );
    assert!(
        index.contains(
            "Only open invoices are listed. (Validation, not scored) — [List](billing/list.md)"
        ),
        "{index}"
    );
    assert_eq!(index.matches("A payment above the limit").count(), 2);

    let agents = content("functional/llms.txt");
    assert!(
        agents.contains("[Business rules by entity](rules.md): 3 rule(s)"),
        "{agents}"
    );
}

#[test]
fn a_rule_of_a_vanished_feature_is_neither_counted_nor_indexed() {
    let (d, f, u) = sample();
    let mut all = rules();
    all[2].slug = "gone-1".to_string();
    let files = render_with_rules(&d, &f, &u, &all, "r", &meta()).unwrap();
    let text: String = files.iter().map(|f| f.content.as_str()).collect();
    assert!(!text.contains("Only open invoices"), "{text}");
    assert!(text.contains("2 rule(s)"), "{text}");
}

#[test]
fn without_rules_the_docs_are_those_of_render() {
    let (d, f, u) = sample();
    let plain = render(&d, &f, &u, "r", &meta()).unwrap();
    assert_eq!(
        render_with_rules(&d, &f, &u, &[], "r", &meta()).unwrap(),
        plain
    );
    assert!(plain.iter().all(|f| !f.content.contains("Business rules")));
    assert!(plain.iter().all(|f| !f.path.ends_with("rules.md")));
}

#[test]
fn rules_render_deterministically_and_a_rerun_is_a_no_op() {
    let (d, f, u) = sample();
    let dir = tempfile::tempdir().unwrap();
    let files = || render_with_rules(&d, &f, &u, &rules(), "r", &meta()).unwrap();
    assert_eq!(files(), files());
    crate::plan(dir.path(), files())
        .unwrap()
        .apply(dir.path())
        .unwrap();
    assert_eq!(crate::plan(dir.path(), files()).unwrap().change_count(), 0);
}
