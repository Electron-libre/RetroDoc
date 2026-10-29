use super::*;

use crate::domains::DomainCluster;
use retrodoc_core::model::RuleEvidence;
use retrodoc_ingest::existing_docs::ExistingDoc;

fn feature(slug: &str, domain: &str, score: Option<f32>) -> Feature {
    Feature {
        back_office: false,
        slug: slug.to_string(),
        domain_slug: domain.to_string(),
        name: slug.to_string(),
        description: String::new(),
        source_paths: vec![],
        entry_points: Vec::new(),
        confidence: score.map(|s| ConfidenceScore::new(s, None)),
    }
}

fn use_case(slug: &str, feature: &str, score: Option<f32>) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.to_string(),
        feature_slug: feature.to_string(),
        name: slug.to_string(),
        description: String::new(),
        steps: vec![],
        diagram_mermaid: None,
        confidence: score.map(|s| ConfidenceScore::new(s, "because".to_string())),
    }
}

#[test]
fn the_behaviour_coverage_lists_unplaced_entry_points_and_technical_support() {
    let mut report = build_report(&DomainMap::default(), &[], &[]);
    assert!(!report.to_markdown().contains("Technical support"));

    report.with_behaviour_coverage(Some(BehaviourCoverage {
        unplaced_entry_points: vec!["a.rb#GET /a".to_string()],
        technical_support: vec![PathBuf::from("helper.rb")],
        by_domain: BTreeMap::from([
            (
                "orders".to_string(),
                EntryCount {
                    entry_points: 40,
                    unplaced: 3,
                },
            ),
            (
                "billing".to_string(),
                EntryCount {
                    entry_points: 5,
                    unplaced: 0,
                },
            ),
        ]),
        by_feature: BTreeMap::from([
            (
                "sign".to_string(),
                FeatureCoverage {
                    entry_points: 4,
                    uncovered: vec!["a.rb#GET /b".to_string(), "a.rb#GET /c".to_string()],
                },
            ),
            (
                "pay".to_string(),
                FeatureCoverage {
                    entry_points: 2,
                    uncovered: Vec::new(),
                },
            ),
        ]),
    }));

    let md = report.to_markdown();
    assert!(md.contains("## Entry points without a use case"), "{md}");
    assert!(md.contains("2 of 6 entry points"), "{md}");
    assert!(
        md.contains("| sign | 4 | 2 |") && !md.contains("| pay |"),
        "{md}"
    );
    assert!(md.contains("`a.rb#GET /c`"));
    assert!(md.contains("## Entry points without a feature"), "{md}");
    assert!(md.contains("`a.rb#GET /a`"));
    assert!(
        md.contains("| orders | 40 | 3 |") && md.contains("| billing | 5 | 0 |"),
        "{md}"
    );
    assert!(
        md.find("| orders |") < md.find("| billing |"),
        "the largest gap first"
    );
    assert!(md.contains("## Technical support (reached by no behaviour)"));
    assert!(md.contains("1 source file(s)"));
    assert!(md.contains("`helper.rb`"));
}

fn rule(slug: &str, statement: &str, evidence: Vec<RuleEvidence>) -> BusinessRule {
    BusinessRule {
        slug: slug.to_string(),
        statement: statement.to_string(),
        kind: retrodoc_core::model::RuleKind::Limit,
        entities: Vec::new(),
        use_cases: Vec::new(),
        evidence,
        confidence: None,
    }
}

fn reconciliation() -> Reconciliation {
    let docs = [ExistingDoc {
        path: PathBuf::from("README.md"),
        content: "# Orders\nAn order is refused when delivery takes an hour.\n".into(),
    }];
    let mut orders = feature("orders", "billing", None);
    orders.entry_points = vec!["a.rb#GET /orders".into()];
    let rules = [
        rule(
            "orders-1",
            "An order is refused when delivery takes an hour or more.",
            vec![],
        ),
        rule(
            "orders-2",
            "Riders above the weekly cap are paused.",
            vec![],
        ),
        rule("gone-1", "Credits expire after a year.", vec![]),
    ];
    crate::reconcile::reconcile(
        &DomainMap::default(),
        &[orders],
        &[],
        &rules,
        &docs,
        &crate::reconcile::Judgements::default(),
    )
}

// spec: REC-9
#[test]
fn the_existing_docs_coverage_is_counted_per_domain() {
    let mut report = build_report(&DomainMap::default(), &[], &[]);
    report.with_reconciliation(Reconciliation::default());
    assert!(!report.to_markdown().contains("## Existing documentation"));

    report.with_reconciliation(reconciliation());

    let md = report.to_markdown();
    assert!(md.contains("## Existing documentation"), "{md}");
    assert!(
        md.contains(
            "| Domain | Items | Documented | Matched, not judged | Undocumented | Contradicted |"
        ),
        "{md}"
    );
    assert!(md.contains("| billing | 3 | 0 | 2 | 1 | 0 |"), "{md}");
    assert!(md.contains("| ? | 1 | 0 | 0 | 1 | 0 |"), "{md}");
}

// spec: REC-10, REC-13
#[test]
fn the_undocumented_items_are_listed_by_importance_rules_among_them() {
    let mut report = build_report(&DomainMap::default(), &[], &[]);
    report.with_reconciliation(reconciliation());

    let md = report.to_markdown();
    assert!(!md.contains("Business rules without documentation"), "{md}");
    assert!(
        md.contains(
            "### Undocumented\n\n2 item(s) are mentioned in no existing document, most entry \
             points first:\n\n\
             - `billing/orders/orders-2` rule: Riders above the weekly cap are paused. (1 entry point)\n\
             - `?/?/gone-1` rule: Credits expire after a year. (0 entry points)\n"
        ),
        "{md}"
    );
}

// spec: REC-11
#[test]
fn each_contradiction_cites_both_sources_as_a_finding_to_check() {
    use crate::reconcile::{Contradiction, ItemKind, ItemVerdict, SectionRef, Status};
    let mut report = build_report(&DomainMap::default(), &[], &[]);
    report.with_reconciliation(Reconciliation {
        items: vec![ItemVerdict {
            kind: ItemKind::UseCase,
            label: "billing/refund/refund-order".into(),
            domain: "billing".into(),
            name: "Refund an order".into(),
            entry_points: 1,
            confidence: Some(0.8),
            status: Status::Contradicted,
            sections: vec![SectionRef::parse("docs/guide.md#Refunds")],
            contradictions: vec![Contradiction {
                section: SectionRef::parse("docs/guide.md#Refunds"),
                doc_states: "Refunds take a week.".into(),
                item_states: "Refunds are made at once.".into(),
            }],
        }],
    });

    let md = report.to_markdown();
    assert!(md.contains("| billing | 1 | 0 | 0 | 0 | 1 |"), "{md}");
    assert!(
        md.contains(
            "### Contradicted\n\nThe existing docs and the code disagree on these items; the doc \
             may be out of date rather than wrong, check which one is right:\n\n\
             - `billing/refund/refund-order` use case: Refund an order\n  \
             - `docs/guide.md#Refunds` states: Refunds take a week.\n  \
             - the generated docs state: Refunds are made at once.\n"
        ),
        "{md}"
    );
}

// spec: REC-10
#[test]
fn at_most_fifty_undocumented_items_are_listed() {
    let rules: Vec<BusinessRule> = (0..55)
        .map(|i| {
            rule(
                &format!("r-{i}"),
                "Riders above the cap are paused.",
                vec![],
            )
        })
        .collect();
    let mut report = build_report(&DomainMap::default(), &[], &[]);
    report.with_reconciliation(crate::reconcile::reconcile(
        &DomainMap::default(),
        &[],
        &[],
        &rules,
        &[],
        &crate::reconcile::Judgements::default(),
    ));
    let md = report.to_markdown();
    assert!(md.contains("- … and 5 more\n"), "{md}");
    assert_eq!(md.matches("rule: Riders").count(), 50);
}

#[test]
fn what_the_rules_pass_set_aside_is_listed_to_be_checked() {
    let mut report = build_report(&DomainMap::default(), &[], &[]);
    report.with_set_aside(SetAside::default(), &RulesConfig::default(), &[]);
    assert!(!report.to_markdown().contains("Set aside by the rules pass"));

    report.with_set_aside(
        SetAside {
            features: vec![AsideFeature {
                slug: "email-verification".into(),
                name: "Verify an email".into(),
                description: "Checks the address\nof a new user".into(),
                reason: "describes how the application is tested".into(),
            }],
            rules: vec![AsideRule {
                slug: "orders-1".into(),
                feature: "orders".into(),
                statement: "An order is stored by the customer id.".into(),
                class: "storage".into(),
                rule: None,
            }],
        },
        &RulesConfig::default(),
        &[],
    );
    let md = report.to_markdown();
    assert!(md.contains("## Set aside by the rules pass"), "{md}");
    assert!(md.contains("`email-verification`"), "{md}");
    assert!(md.contains("Checks the address of a new user"), "{md}");
    assert!(
        md.contains("describes how the application is tested"),
        "{md}"
    );
    assert!(md.contains("`orders-1` (storage)"), "{md}");
    assert!(md.contains("rules-set-aside.yaml"), "{md}");
}

// spec: RSA-7
#[test]
fn the_set_aside_lists_say_how_to_put_an_entry_back_and_mark_the_rules_put_back() {
    let aside = |slug: &str| AsideRule {
        slug: slug.into(),
        feature: "orders".into(),
        statement: "An order is stored by the customer id.".into(),
        class: "storage".into(),
        rule: None,
    };
    let published = |slug: &str| rule(slug, "An order is stored.", Vec::new());
    let mut report = build_report(&DomainMap::default(), &[], &[]);
    report.with_set_aside(
        SetAside {
            features: vec![AsideFeature {
                slug: "email-verification".into(),
                name: "Verify an email".into(),
                description: "Checks the address".into(),
                reason: "describes how the application is tested".into(),
            }],
            rules: vec![aside("orders-1"), aside("orders-3"), aside("orders-4")],
        },
        &RulesConfig {
            keep_features: Vec::new(),
            keep_rules: vec!["orders-3".into(), "orders-4".into(), "ghost-1".into()],
        },
        // `orders-4` is listed but not published (an older file, or `generate` not run since).
        &[published("orders-2"), published("orders-3")],
    );

    let md = report.to_markdown();
    assert!(
        md.contains("add its slug to `keep_features` in the `[rules]` section of `retrodoc.toml`"),
        "{md}"
    );
    assert!(
        md.contains("add its slug to `keep_rules` in the `[rules]` section of `retrodoc.toml`"),
        "{md}"
    );
    assert!(md.contains("- `orders-1` (storage) An order"), "{md}");
    assert!(
        md.contains("- `orders-3` (storage, put back by hand) An order"),
        "{md}"
    );
    assert!(!md.contains("ghost-1"), "{md}");
    assert!(md.contains("- `orders-4` (storage) An order"), "{md}");
}

#[test]
fn aggregates_per_domain_and_lists_debt() {
    let domains = DomainMap {
        domains: vec![DomainCluster {
            slug: UNCATEGORIZED_SLUG.to_string(),
            name: String::new(),
            description: String::new(),
            paths: vec![PathBuf::from("x.rs")],
            ..DomainCluster::default()
        }],
    };
    let features = vec![
        feature("pay", "billing", Some(0.2)),
        feature("login", "auth", Some(0.9)),
        feature("orphan", "auth", Some(0.0)),
    ];
    let use_cases = vec![
        use_case("pay-invoice", "pay", Some(0.2)),
        use_case("sign-in", "login", Some(0.9)),
        use_case("mystery", "login", None),
    ];

    let report = build_report(&domains, &features, &use_cases);

    assert!((report.overall.unwrap() - 0.55).abs() < 1e-6);
    // Least trusted domain first: billing 0.2, auth (0.9 + 0.0) / 2.
    assert_eq!(report.domains[0].slug, "billing");
    assert_eq!(report.domains[1].slug, "auth");
    assert!((report.domains[1].confidence.unwrap() - 0.45).abs() < 1e-6);
    assert_eq!(
        (report.domains[1].features, report.domains[1].use_cases),
        (2, 2)
    );
    // Lowest first; "orphan" has no use case so it is listed itself.
    let labels: Vec<&str> = report
        .weak_sections
        .iter()
        .map(|w| w.label.as_str())
        .collect();
    assert_eq!(labels, ["auth/orphan", "billing/pay/pay-invoice"]);
    assert_eq!(report.unscored_use_cases, ["auth/login/mystery"]);
    assert_eq!(report.uncategorized_files, [PathBuf::from("x.rs")]);

    let mut report = report;
    report.skipped_files = vec![PathBuf::from("left.rs")];
    assert!(report
        .to_markdown()
        .contains("## Not analysed (file budget)"));
    assert!(report.to_markdown().contains("`left.rs`"));

    let md = report.to_markdown();
    assert!(md.contains("Overall confidence: **55%**"));
    assert!(md.contains("`billing/pay/pay-invoice` — 20%: because"));
    assert!(md.contains("`x.rs`"));
}

#[test]
fn technical_sounding_use_cases_get_their_own_section() {
    let mut readable = use_case("readable", "pay", Some(0.9));
    readable.business_language = Some(ConfidenceScore::new(0.9, None));
    let mut technical = use_case("helper", "pay", Some(0.9));
    technical.business_language = Some(ConfidenceScore::new(
        0.2,
        "code-level wording in the narrative: controller".to_string(),
    ));
    let features = [feature("pay", "billing", Some(0.9))];

    let report = build_report(&DomainMap::default(), &features, &[readable, technical]);

    assert_eq!(report.technical_use_cases.len(), 1);
    assert_eq!(report.technical_use_cases[0].label, "billing/pay/helper");
    // 0.2 is under both the debt threshold and the one that sets a use case apart.
    assert_eq!(
        (report.set_apart_use_cases, report.back_office_features),
        (1, 0)
    );
    assert!((report.overall_business.unwrap() - 0.55).abs() < 1e-6);
    let md = report.to_markdown();
    assert!(md.contains("Overall business language: **55%**"));
    assert!(md.contains("## Technical-sounding use cases (business language below 50%)"));
    assert!(
        md.contains("`billing/pay/helper` — 20%: code-level wording in the narrative: controller")
    );
    // Not scored (older artifacts): no business section at all.
    let plain = build_report(
        &DomainMap::default(),
        &features,
        &[use_case("a", "pay", Some(0.9))],
    );
    assert!(!plain.to_markdown().contains("business language"));
}
