use retrodoc_core::model::{BusinessRule, ConfidenceScore, RuleEvidence, RuleKind, SourceRef};
use retrodoc_pipeline::{DomainMap, Glossary};

use super::Docs;
use crate::corpus::fixtures::*;
use crate::edits::{self, Edit, Fields};
use crate::freshness::Freshness;
use crate::read_model::GeneratedDocs;
use crate::reviews::tests::repo;
use crate::reviews::{fingerprint, record, Section, SectionKind, Status};

fn docs() -> Docs {
    let mut weak = use_case(
        "pay-late",
        "pay-invoice",
        "Pay late",
        "Settled after the due date",
    );
    weak.confidence = Some(ConfidenceScore::new(
        0.3,
        "no code found for step 2".to_string(),
    ));
    Docs::new(
        vec![domain("billing", "Billing", "Invoices and payments")],
        vec![
            feature("pay-invoice", "Pay an invoice", "Settle what is due"),
            feature("cancel-plan", "Cancel a plan", "Stop a subscription"),
        ],
        vec![
            use_case(
                "pay-by-card",
                "pay-invoice",
                "Pay by card",
                "The accountant settles it",
            ),
            weak,
        ],
        &Glossary::default(),
        &[],
    )
}

#[test]
fn domains_are_listed_with_their_feature_counts() {
    let out = docs().list_domains();
    assert!(
        out.contains("- `billing` — Billing: Invoices and payments (confidence 80%, 2 feature(s))")
    );
}

// spec: MCP-6 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_name_over_several_lines_forges_no_item_and_a_leading_blank_line_keeps_the_text() {
    let mut paid = use_case("pay-by-card", "pay-invoice", "Pay\nby card", "Settled");
    paid.description = "\nThe accountant settles it".into();
    let docs = Docs::new(
        vec![domain(
            "billing",
            "Billing\n- `forged` — Fake",
            "\nInvoices and payments",
        )],
        vec![feature(
            "pay-invoice",
            "Pay an\ninvoice",
            "\n\nSettle what is due",
        )],
        vec![paid],
        &Glossary::default(),
        &[],
    );
    let domains = docs.list_domains();
    assert!(
        domains.contains("- `billing` — Billing - `forged` — Fake: Invoices and payments ("),
        "{domains}"
    );
    let domain = docs.get_domain("billing");
    assert!(
        domain.contains("— Pay an invoice: Settle what is due ("),
        "{domain}"
    );
    let feature = docs.get_feature("billing/pay-invoice");
    assert!(
        feature.contains("— Pay by card: The accountant settles it ("),
        "{feature}"
    );
}

#[test]
fn a_domain_lists_its_features_with_ids_to_follow() {
    let out = docs().get_domain("billing");
    assert!(out.starts_with("# Billing (`billing`)"));
    assert!(out.contains("`billing/pay-invoice` — Pay an invoice"));
    assert!(out.contains("`billing/cancel-plan`"));
}

#[test]
fn a_domain_without_feature_says_so() {
    let docs = Docs::new(
        vec![domain("billing", "Billing", "Invoices and payments")],
        vec![],
        vec![],
        &Glossary::default(),
        &[],
    );
    let out = docs.get_domain("billing");
    assert!(out.contains("Invoices and payments"));
    assert!(out.contains("No feature documented here."));
}

// spec: MCP-1 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_domain_is_addressed_by_its_slug_alone() {
    let out = docs().get_domain("billing/payment");
    assert!(
        out.starts_with("Not documented: no domain `billing/payment`."),
        "{out}"
    );
}

#[test]
fn a_feature_shows_its_use_cases_and_cited_files() {
    let out = docs().get_feature("billing/pay-invoice");
    assert!(out.contains("Grounded on: app/invoice.rb"));
    assert!(out.contains("`billing/pay-invoice/pay-by-card` — Pay by card"));
    assert!(out.contains("Pay late"));
    assert!(docs()
        .get_feature("pay-invoice")
        .contains("# Pay an invoice"));
    assert!(docs()
        .get_feature("cancel-plan")
        .contains("No use case documented"));
}

#[test]
fn what_the_docs_set_apart_is_marked_in_the_answers() {
    // `pay-late` has a confidence of 30%, `pay-by-card` 90%.
    let feature = docs().get_feature("billing/pay-invoice");
    let late = feature.lines().find(|l| l.contains("Pay late")).unwrap();
    let card = feature.lines().find(|l| l.contains("Pay by card")).unwrap();
    assert!(late.contains("[administration/technical]"));
    assert!(!card.contains("[administration/technical]"));
    assert!(docs()
        .get_use_case("pay-invoice/pay-late")
        .contains("Administration and technical"));
    assert!(!docs()
        .get_use_case("billing/pay-invoice/pay-by-card")
        .contains("Administration and technical"));
    let found = docs().search_docs("late", 10);
    let hit = found.lines().find(|l| l.contains("Pay late")).unwrap();
    assert!(hit.contains("[administration/technical]"), "{found}");
}

#[test]
fn a_back_office_feature_is_marked_with_its_use_cases() {
    let mut docs = docs();
    docs.generated
        .features_mut()
        .iter_mut()
        .find(|f| f.slug == "pay-invoice")
        .unwrap()
        .back_office = true;
    let out = docs.get_use_case("billing/pay-invoice/pay-by-card");
    assert!(out.contains("Administration and technical"));
    assert!(docs
        .get_feature("billing/pay-invoice")
        .contains("Administration and technical"));
}

#[test]
fn a_use_case_gives_narrative_steps_actor_and_code() {
    let out = docs().get_use_case("billing/pay-invoice/pay-by-card");
    assert!(out.contains("The accountant settles it"));
    assert!(out.contains("Primary actor: Accountant"));
    assert!(out.contains("Triggered by: POST /invoices/:id/pay"));
    assert!(out.contains("1. Accountant — records: Checks the amount (`app/invoice.rb`)"));
    assert!(out.contains("Confidence: 90%"));
    assert!(!out.contains("verify against the code"));
}

#[test]
fn a_low_confidence_answer_tells_the_agent_to_check_the_code() {
    let out = docs().get_use_case("pay-invoice/pay-late");
    assert!(
        out.contains("Confidence: 30% (no code found for step 2) — low: verify against the code")
    );
    assert!(docs()
        .get_feature("pay-invoice")
        .contains("confidence 30% (low)"));
}

// spec: MCP-5 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn an_unknown_id_is_not_documented_and_points_to_the_way_out() {
    let docs = docs();
    let out = docs.get_domain("shipping");
    assert!(out.starts_with("Not documented: no domain `shipping`."));
    assert!(out.contains("Known domains: billing."));
    assert!(docs
        .get_feature("x")
        .contains("Not documented: no feature `x`"));
    assert!(docs
        .get_use_case("billing/pay-invoice/nope")
        .contains("Not documented: no use case"));
    assert!(docs
        .get_use_case("cancel-plan/pay-by-card")
        .contains("Not documented"));
}

#[test]
fn a_search_lists_ids_confidence_and_files_or_says_not_documented() {
    let docs = docs();
    let out = docs.search_docs("cancel subscription", 5);
    assert!(out.contains("- [feature] Cancel a plan (`billing/cancel-plan`, not scored)"));
    let out = docs.search_docs("zzzz", 5);
    assert!(out.starts_with("Not documented: nothing among"));
}

// spec: MCP-5 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn an_unknown_domain_among_none_names_no_known_domain() {
    let docs = Docs::new(vec![], vec![], vec![], &Glossary::default(), &[]);
    let out = docs.get_domain("billing");
    assert!(
        out.starts_with("Not documented: no domain `billing`."),
        "{out}"
    );
    assert!(!out.contains("Known domains"), "{out}");
}

// spec: MCP-2 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_feature_id_whose_domain_is_wrong_is_not_documented() {
    let docs = docs();
    let out = docs.get_feature("shipping/pay-invoice");
    assert!(
        out.starts_with("Not documented: no feature `shipping/pay-invoice`."),
        "{out}"
    );
    assert!(docs
        .get_feature("billing/pay-invoice")
        .starts_with("# Pay an invoice"));
    assert!(docs
        .get_feature("pay-invoice")
        .starts_with("# Pay an invoice"));
}

// spec: MCP-3 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_use_case_id_whose_feature_or_domain_is_wrong_is_not_documented() {
    let docs = docs();
    for id in [
        "shipping/pay-invoice/pay-by-card",
        "cancel-plan/pay-by-card",
        "billing/cancel-plan/pay-by-card",
    ] {
        let out = docs.get_use_case(id);
        assert!(
            out.starts_with("Not documented: no use case"),
            "{id}: {out}"
        );
    }
    assert!(docs
        .get_use_case("billing/pay-invoice/pay-by-card")
        .starts_with("# Pay by card"));
    assert!(docs
        .get_use_case("pay-invoice/pay-by-card")
        .starts_with("# Pay by card"));
}

// spec: MCP-4 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_use_case_slug_alone_answers_its_use_case_or_lists_the_candidates() {
    let docs = Docs::new(
        vec![domain("billing", "Billing", "Invoices and payments")],
        vec![
            feature("pay-invoice", "Pay an invoice", "Settle what is due"),
            feature("pay-order", "Pay an order", "Settle an order"),
        ],
        vec![
            use_case("pay-by-card", "pay-invoice", "Pay by card", "Settled"),
            use_case("pay-by-card", "pay-order", "Pay by card", "Settled"),
            use_case("pay-late", "pay-invoice", "Pay late", "Settled late"),
        ],
        &Glossary::default(),
        &[],
    );
    assert!(docs.get_use_case("pay-late").starts_with("# Pay late"));
    let out = docs.get_use_case("pay-by-card");
    assert!(out.starts_with("Not documented:"), "{out}");
    assert!(out.contains("`billing/pay-invoice/pay-by-card`"), "{out}");
    assert!(out.contains("`billing/pay-order/pay-by-card`"), "{out}");
}

// spec: DSR-3, DSR-4, DSR-5 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_search_on_common_words_only_says_not_documented_and_how_many_were_left_out() {
    let docs = docs();
    let out = docs.search_docs("pay salary", 5);
    assert!(out.starts_with("Not documented:"), "{out}");
    assert!(
        out.contains("entries share only part of the query"),
        "{out}"
    );
    assert!(out.contains("no entry has `salary`"), "{out}");
    assert!(out.contains("Try other words"), "{out}");
    assert!(!docs.search_docs("zzzz", 5).contains("share only"));
}

#[test]
fn nothing_is_loaded_before_a_first_generate() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Docs::load(dir.path(), &[]).is_none());
}

#[test]
fn the_saved_artifacts_are_loaded_and_navigable() {
    let dir = tempfile::tempdir().unwrap();
    retrodoc_pipeline::save_features(
        dir.path(),
        &[feature(
            "pay-invoice",
            "Pay an invoice",
            "Settle what is due",
        )],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(
        dir.path(),
        &[use_case("pay-by-card", "pay-invoice", "Pay by card", "x")],
    )
    .unwrap();
    DomainMap {
        domains: vec![retrodoc_pipeline::DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            description: "Invoices".into(),
            paths: vec![],
            ..retrodoc_pipeline::DomainCluster::default()
        }],
    }
    .save(dir.path())
    .unwrap();

    let docs = Docs::load(dir.path(), &[]).unwrap();
    assert!(docs.get_domain("billing").contains("`billing/pay-invoice`"));
    assert!(docs.get_use_case("pay-by-card").contains("Pay by card"));
    assert_eq!(docs.index().len(), 3);
}

/// The docs of `docs()` over a repo where `generate` recorded the hashes
/// of the two files they cite.
fn docs_over_a_generated_repo() -> (tempfile::TempDir, Docs) {
    let dir = tempfile::tempdir().unwrap();
    let mut cache = retrodoc_pipeline::SourceHashes::default();
    for name in ["app/invoice.rb", "app/mailer.rb"] {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("content of {name}")).unwrap();
        cache.put(
            name,
            &retrodoc_pipeline::cache::hash_content(&format!("content of {name}")),
        );
    }
    cache.save(dir.path()).unwrap();
    let docs = docs().with_freshness(Freshness::load(dir.path()));
    (dir, docs)
}

#[test]
fn untouched_code_gives_answers_without_warning() {
    let (_dir, docs) = docs_over_a_generated_repo();
    for out in [
        docs.get_feature("pay-invoice"),
        docs.get_use_case("billing/pay-invoice/pay-by-card"),
        docs.search_docs("settle invoice", 5),
    ] {
        assert!(!out.contains("tale"), "{out}");
    }
}

#[test]
fn a_use_case_warns_about_the_cited_files_that_changed_and_only_those() {
    let (dir, docs) = docs_over_a_generated_repo();
    std::fs::write(dir.path().join("app/mailer.rb"), "edited").unwrap();
    let out = docs.get_use_case("billing/pay-invoice/pay-by-card");
    assert!(out.contains("⚠ Stale"), "{out}");
    assert!(out.contains("`app/mailer.rb` (modified)"), "{out}");
    assert!(!out.contains("`app/invoice.rb` (modified)"), "{out}");
    // The feature cites only the file that did not change.
    assert!(!docs.get_feature("pay-invoice").contains("Stale"));
}

#[test]
fn a_feature_and_a_search_hit_warn_about_a_deleted_file() {
    let (dir, docs) = docs_over_a_generated_repo();
    std::fs::remove_file(dir.path().join("app/invoice.rb")).unwrap();
    let out = docs.get_feature("pay-invoice");
    assert!(out.contains("⚠ Stale"), "{out}");
    assert!(out.contains("`app/invoice.rb` (deleted)"), "{out}");
    let out = docs.search_docs("settle invoice", 5);
    assert!(out.contains("⚠ stale: `app/invoice.rb` (deleted)"), "{out}");
}

#[test]
fn docs_loaded_without_a_recorded_state_never_warn() {
    assert!(!docs()
        .get_use_case("pay-invoice/pay-by-card")
        .contains("tale"));
}

#[test]
fn the_cited_code_can_be_read_through_the_docs_and_nothing_else() {
    let (dir, docs) = docs_over_a_generated_repo();
    let docs = docs.with_source_access(dir.path());
    let out = docs.read_source("app/invoice.rb", None, None);
    assert!(
        out.starts_with("# `app/invoice.rb` (lines 1-1 of 1)"),
        "{out}"
    );
    assert!(out.contains("   1 | content of app/invoice.rb"), "{out}");
    assert!(!out.contains("Stale"), "{out}");
    std::fs::write(dir.path().join("README.md"), "docs").unwrap();
    assert!(docs
        .read_source("README.md", None, None)
        .starts_with("Not available:"));
    assert!(docs
        .read_source("../x", None, None)
        .starts_with("Not available:"));
    assert!(docs.git_log("README.md", 5).starts_with("Not available:"));
}

#[test]
fn a_file_read_after_it_changed_carries_the_stale_notice() {
    let (dir, docs) = docs_over_a_generated_repo();
    let docs = docs.with_source_access(dir.path());
    std::fs::write(dir.path().join("app/mailer.rb"), "edited").unwrap();
    let out = docs.read_source("app/mailer.rb", None, None);
    assert!(out.contains("⚠ Stale"), "{out}");
    assert!(out.contains("   1 | edited"), "{out}");
}

#[test]
fn without_access_to_the_repository_the_code_tools_say_so() {
    let out = docs().read_source("app/invoice.rb", None, None);
    assert!(
        out.starts_with("Not available: this server has no access"),
        "{out}"
    );
    assert!(docs()
        .git_log("app/invoice.rb", 5)
        .starts_with("Not available:"));
}

fn rules() -> Vec<BusinessRule> {
    vec![
        BusinessRule {
            slug: "pay-invoice-1".into(),
            statement: "A paid invoice cannot be paid again.".into(),
            kind: RuleKind::StateTransition,
            entities: vec!["Invoice".into()],
            use_cases: vec!["pay-by-card".into()],
            evidence: vec![
                RuleEvidence::Code {
                    source: SourceRef {
                        path: "app/mailer.rb".into(),
                        start_line: Some(3),
                        end_line: Some(9),
                    },
                },
                RuleEvidence::Test {
                    phrase: "refuses a second payment".into(),
                    path: "spec/pay_spec.rb".into(),
                },
            ],
            confidence: Some(ConfidenceScore::new(0.7, None)),
        },
        BusinessRule {
            slug: "pay-invoice-2".into(),
            statement: "Amounts above the credit ceiling need approval.".into(),
            kind: RuleKind::Limit,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: vec![RuleEvidence::Code {
                source: SourceRef {
                    path: "app/invoice.rb".into(),
                    start_line: None,
                    end_line: None,
                },
            }],
            confidence: Some(ConfidenceScore::new(0.4, None)),
        },
        BusinessRule {
            slug: "cancel-plan-1".into(),
            statement: "A cancelled plan stops renewing.".into(),
            kind: RuleKind::Computation,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: Vec::new(),
            confidence: None,
        },
    ]
}

#[test]
fn rules_are_in_the_feature_use_case_and_domain_answers() {
    let docs = docs().with_rules(rules());

    let feature = docs.get_feature("pay-invoice");
    assert!(feature.contains("## Business rules"), "{feature}");
    assert!(
        feature.contains(
            "- `pay-invoice-1` — A paid invoice cannot be paid again. (state transition, confidence 70%)"
        ),
        "{feature}"
    );
    assert!(feature.contains("  - Entities: Invoice"), "{feature}");
    assert!(
        feature.contains("  - Code: `app/mailer.rb:3-9`"),
        "{feature}"
    );
    assert!(
        feature.contains("  - Test: \"refuses a second payment\" (`spec/pay_spec.rb`)"),
        "{feature}"
    );
    assert!(
        feature.contains("(limit, confidence 40% (low))"),
        "{feature}"
    );
    assert!(!feature.contains("cancelled plan"), "{feature}");

    let use_case = docs.get_use_case("billing/pay-invoice/pay-by-card");
    assert!(
        use_case.contains("paid invoice cannot be paid again"),
        "{use_case}"
    );
    assert!(!use_case.contains("credit ceiling"), "{use_case}");

    let domain = docs.get_domain("billing");
    assert!(domain.contains("## Business rules"), "{domain}");
    assert!(domain.contains("`pay-invoice-2`"), "{domain}");
    assert!(domain.contains("`cancel-plan-1`"), "{domain}");
}

#[test]
fn a_feature_without_use_case_still_shows_its_rules() {
    let docs = docs().with_rules(rules());
    let out = docs.get_feature("cancel-plan");
    assert!(out.contains("No use case documented"), "{out}");
    assert!(out.contains("A cancelled plan stops renewing."), "{out}");
}

#[test]
fn without_rules_the_answers_have_no_rules_section() {
    let docs = docs();
    for out in [
        docs.get_feature("pay-invoice"),
        docs.get_use_case("pay-by-card"),
        docs.get_domain("billing"),
    ] {
        assert!(!out.contains("Business rules"), "{out}");
    }
}

#[test]
fn a_search_finds_a_rule_by_what_it_says_and_by_its_tests() {
    let docs = docs().with_rules(rules());
    let out = docs.search_docs("credit ceiling approval", 3);
    assert!(
        out.contains("[rule] Amounts above the credit ceiling need approval."),
        "{out}"
    );
    assert!(out.contains("`pay-invoice-2`"), "{out}");
    let out = docs.search_docs("second payment", 3);
    assert!(out.contains("`pay-invoice-1`"), "{out}");
    assert!(
        out.contains("feature: `billing/pay-invoice` (see `get_feature`)"),
        "{out}"
    );
    assert!(out.contains("files: app/mailer.rb"), "{out}");
}

#[test]
fn a_rule_warns_when_the_code_it_cites_changed_and_its_code_can_be_read() {
    let (dir, docs) = docs_over_a_generated_repo();
    let docs = docs.with_rules(rules()).with_source_access(dir.path());
    assert!(!docs.get_feature("pay-invoice").contains("tale"));
    std::fs::write(dir.path().join("app/mailer.rb"), "edited").unwrap();
    let out = docs.get_feature("pay-invoice");
    assert!(
        out.contains("  - ⚠ stale: `app/mailer.rb` (modified)"),
        "{out}"
    );
    assert!(docs
        .read_source("app/mailer.rb", None, None)
        .starts_with("# `app/mailer.rb`"));
}

#[test]
fn the_code_only_a_rule_cites_can_be_read_whatever_the_order_of_the_builders() {
    let (dir, docs) = docs_over_a_generated_repo();
    std::fs::write(dir.path().join("app/ceiling.rb"), "limit").unwrap();
    let mut rules = rules();
    rules[1].evidence = vec![RuleEvidence::Code {
        source: SourceRef {
            path: "app/ceiling.rb".into(),
            start_line: None,
            end_line: None,
        },
    }];
    let docs = docs
        .with_source_access(dir.path())
        .with_rules(rules.clone());
    let out = docs.read_source("app/ceiling.rb", None, None);
    assert!(out.starts_with("# `app/ceiling.rb`"), "{out}");
    let docs = docs.with_rules(Vec::new()).with_rules(rules);
    let out = docs.read_source("app/ceiling.rb", None, None);
    assert!(out.starts_with("# `app/ceiling.rb`"), "{out}");
}

#[test]
fn docs_made_from_a_read_model_with_a_repository_can_read_its_code() {
    let (dir, _) = docs_over_a_generated_repo();
    let generated = GeneratedDocs::new(
        vec![domain("billing", "Billing", "Invoices and payments")],
        vec![feature(
            "pay-invoice",
            "Pay an invoice",
            "Settle what is due",
        )],
        vec![],
        Glossary::default(),
        &[],
    )
    .with_repo_root(dir.path());
    let out = Docs::from(generated).read_source("app/invoice.rb", None, None);
    assert!(out.starts_with("# `app/invoice.rb`"), "{out}");
}

#[test]
fn a_set_apart_use_case_of_an_undocumented_feature_is_marked_in_the_search() {
    let mut weak = use_case("pay-late", "pay-invoice", "Pay late", "Settled late");
    weak.confidence = Some(ConfidenceScore::new(0.3, None));
    let docs = Docs::new(vec![], vec![], vec![weak], &Glossary::default(), &[]);
    let found = docs.search_docs("late", 10);
    let hit = found.lines().find(|l| l.contains("Pay late")).unwrap();
    assert!(hit.contains("`pay-invoice/pay-late`"), "{found}");
    assert!(hit.contains("[administration/technical]"), "{found}");
}

#[test]
fn a_second_set_of_rules_replaces_the_first() {
    let docs = docs()
        .with_rules(rules())
        .with_rules(vec![rules().remove(2)]);
    let out = docs.search_docs("credit ceiling approval", 3);
    assert!(out.starts_with("Not documented"), "{out}");
    assert!(docs
        .search_docs("cancelled plan renewing", 3)
        .contains("`cancel-plan-1`"));
}

#[test]
fn the_saved_rules_are_loaded_with_the_docs() {
    let dir = tempfile::tempdir().unwrap();
    retrodoc_pipeline::save_features(
        dir.path(),
        &[feature(
            "pay-invoice",
            "Pay an invoice",
            "Settle what is due",
        )],
    )
    .unwrap();
    retrodoc_pipeline::save_rules(dir.path(), &rules()[..1]).unwrap();
    let docs = Docs::load(dir.path(), &[]).unwrap();
    assert!(docs.get_feature("pay-invoice").contains("`pay-invoice-1`"));
    assert_eq!(docs.index().len(), 2);
}

/// `docs()` over a temporary repository whose `.retrodoc/reviews.json` holds `reviews` (JSON
/// objects of the file's format), a fingerprint `"now"` replaced by the one of its section in
/// the docs, so that the review is up to date.
fn reviewed_docs(reviews: &[&str]) -> (tempfile::TempDir, Docs) {
    let dir = tempfile::tempdir().unwrap();
    let docs = docs().with_source_access(dir.path());
    let reviews: Vec<String> = reviews
        .iter()
        .map(|review| {
            let mut json: serde_json::Value = serde_json::from_str(review).unwrap();
            if json["fingerprint"] == "now" {
                let section: Section = serde_json::from_value(json.clone()).unwrap();
                json["fingerprint"] = fingerprint(&docs.generated, &section).unwrap().into();
            }
            json.to_string()
        })
        .collect();
    write_reviews(
        dir.path(),
        &reviews.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    (dir, docs)
}

fn write_reviews(root: &std::path::Path, reviews: &[&str]) {
    std::fs::create_dir_all(root.join(".retrodoc")).unwrap();
    std::fs::write(
        root.join(".retrodoc/reviews.json"),
        format!(r#"{{"version": 1, "reviews": [{}]}}"#, reviews.join(",")),
    )
    .unwrap();
}

// spec: MCP-7 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_detail_answer_shows_the_review_of_its_section_under_the_confidence() {
    let (_dir, docs) = reviewed_docs(&[
        r#"{"kind": "domain", "id": "billing", "status": "to_review", "comment": "check the refunds",
            "author": "Ada", "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "now"}"#,
        r#"{"kind": "feature", "id": "pay-invoice", "status": "validated",
            "author": null, "reviewed_at": "2026-11-03T09:00:00Z", "fingerprint": "now"}"#,
        r#"{"kind": "use_case", "id": "pay-invoice/pay-by-card", "status": "validated",
            "comment": "", "author": "Bob", "reviewed_at": "2026-11-02T08:00:00Z", "fingerprint": "now"}"#,
    ]);
    let domain = docs.get_domain("billing");
    assert!(
        domain.contains(
            "Confidence: 80%\nReview: to review — \"check the refunds\" (Ada, 2026-11-04T10:00:00Z)\n"
        ),
        "{domain}"
    );
    let feature = docs.get_feature("pay-invoice");
    assert!(
        feature.contains("\nReview: validated (2026-11-03T09:00:00Z)\n"),
        "{feature}"
    );
    // The slug alone names the same section as `pay-invoice/pay-by-card`.
    let use_case = docs.get_use_case("pay-by-card");
    assert!(
        use_case.contains("\nReview: validated (Bob, 2026-11-02T08:00:00Z)\n"),
        "{use_case}"
    );
    let unreviewed = docs.get_use_case("pay-late");
    assert!(
        unreviewed.contains("\nReview: not reviewed\n"),
        "{unreviewed}"
    );
}

// spec: MCP-8 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_section_marked_wrong_opens_with_a_warning_and_another_status_does_not() {
    let (_dir, docs) = reviewed_docs(&[
        r#"{"kind": "feature", "id": "pay-invoice", "status": "wrong", "comment": "it pays orders",
            "author": "Ada", "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "now"}"#,
        r#"{"kind": "use_case", "id": "pay-invoice/pay-late", "status": "wrong",
            "author": "Ada", "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "now"}"#,
        r#"{"kind": "domain", "id": "billing", "status": "to_review",
            "author": "Ada", "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "now"}"#,
    ]);
    let feature = docs.get_feature("billing/pay-invoice");
    assert!(
        feature.starts_with(
            "⚠ Marked wrong: a human reviewer marked this section wrong: \"it pays orders\". \
             Do not rely on it.\n\n# Pay an invoice"
        ),
        "{feature}"
    );
    let use_case = docs.get_use_case("pay-invoice/pay-late");
    assert!(
        use_case.starts_with(
            "⚠ Marked wrong: a human reviewer marked this section wrong. Do not rely on it.\n\n# "
        ),
        "{use_case}"
    );
    assert!(docs.get_domain("billing").starts_with("# Billing"));
}

// spec: MCP-9 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_review_saved_after_the_docs_were_loaded_shows_in_the_next_answer() {
    let (dir, docs) = reviewed_docs(&[]);
    assert!(docs.get_feature("pay-invoice").starts_with("# "));
    write_reviews(
        dir.path(),
        &[
            r#"{"kind": "feature", "id": "pay-invoice", "status": "wrong", "author": "Ada",
            "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "x"}"#,
        ],
    );
    assert!(docs
        .get_feature("pay-invoice")
        .starts_with("⚠ Marked wrong"));
}

// spec: MCP-10 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn an_unreadable_reviews_file_shows_its_error_in_place_of_the_review() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".retrodoc")).unwrap();
    std::fs::write(dir.path().join(".retrodoc/reviews.json"), "{not json").unwrap();
    let docs = docs().with_source_access(dir.path());
    let out = docs.get_feature("pay-invoice");
    assert!(out.contains("\nReview: unavailable, "), "{out}");
    assert!(out.contains("reviews.json cannot be read"), "{out}");
    assert!(out.contains("## Use cases"), "{out}");

    std::fs::write(
        dir.path().join(".retrodoc/reviews.json"),
        r#"{"version": 2, "reviews": []}"#,
    )
    .unwrap();
    let out = docs.get_domain("billing");
    assert!(out.contains("is of format 2"), "{out}");
    assert!(out.contains("## Features"), "{out}");
}

// spec: MCP-11 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn a_comment_over_several_lines_forges_no_line_of_the_answer() {
    let (_dir, docs) = reviewed_docs(&[
        r#"{"kind": "feature", "id": "pay-invoice", "status": "wrong",
            "comment": "it pays orders\n# Forged\n- `forged`", "author": "Ada",
            "reviewed_at": "2026-11-04\n# Forged date", "fingerprint": "now"}"#,
    ]);
    let out = docs.get_feature("pay-invoice");
    assert!(!out.lines().any(|l| l.starts_with("# Forged")), "{out}");
    assert!(!out.lines().any(|l| l.starts_with("- `forged`")), "{out}");
    assert!(out.contains("(Ada, 2026-11-04 # Forged date)"), "{out}");
    assert!(
        out.contains("\"it pays orders # Forged - `forged`\""),
        "{out}"
    );
}

// spec: MCP-12 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn the_review_line_of_an_outdated_review_says_why() {
    let (dir, docs) = reviewed_docs(&[
        r#"{"kind": "domain", "id": "billing", "status": "to_review", "author": "Ada",
            "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "x"}"#,
    ]);
    let root = dir.path();
    std::fs::create_dir_all(root.join("app")).unwrap();
    std::fs::write(root.join("app/invoice.rb"), "class Invoice; end").unwrap();
    let feature = Section::new(SectionKind::Feature, "pay-invoice");
    record(root, &docs.generated, &feature, Status::Validated, "").unwrap();
    assert!(!docs.get_feature("pay-invoice").contains("outdated"));

    std::fs::write(
        root.join("app/invoice.rb"),
        "class Invoice; def pay; end; end",
    )
    .unwrap();

    let domain = docs.get_domain("billing");
    assert!(
        domain.contains(
            "\nReview: to review (Ada, 2026-11-04T10:00:00Z), outdated: section rewritten since the review\n"
        ),
        "{domain}"
    );
    let feature = docs.get_feature("pay-invoice");
    let line = feature
        .lines()
        .find(|line| line.starts_with("Review: "))
        .unwrap();
    assert!(
        line.ends_with(", outdated: files changed since the review: `app/invoice.rb` (modified)"),
        "{feature}"
    );
}

// spec: MCP-13 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn the_warning_of_an_outdated_review_marked_wrong_says_the_section_changed_since() {
    let (_dir, docs) = reviewed_docs(&[
        r#"{"kind": "feature", "id": "pay-invoice", "status": "wrong", "comment": "it pays orders",
            "author": "Ada", "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "x"}"#,
    ]);
    let feature = docs.get_feature("pay-invoice");
    assert!(
        feature.starts_with(
            "⚠ Marked wrong: a human reviewer marked this section wrong: \"it pays orders\". \
             The section changed since (section rewritten since the review): check whether it still \
             holds before relying on it.\n\n# Pay an invoice"
        ),
        "{feature}"
    );
}

fn edited(docs: Docs, root: &std::path::Path, section: &Section, fields: Fields) -> (Docs, Edit) {
    let edit = edits::record(root, &docs.generated, section, fields).unwrap();
    (docs.with_edits(edits::load(root).unwrap()), edit)
}

// spec: MCP-15 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn the_answer_of_an_edited_section_says_it_is_a_human_edit() {
    let dir = repo("Ada");
    let narrative = Fields {
        narrative: Some("The clerk pays at the desk.".into()),
        ..Fields::default()
    };
    let (docs, edit) = edited(
        docs().with_source_access(dir.path()),
        dir.path(),
        &Section::use_case("pay-invoice", "pay-by-card"),
        narrative,
    );

    let out = docs.get_use_case("pay-by-card");

    assert!(out.contains("\nThe clerk pays at the desk.\n"), "{out}");
    assert!(
        out.contains(&format!(
            "\nReview: not reviewed\nEdit: human edit of the narrative (Ada, {})\n",
            edit.edited_at
        )),
        "{out}"
    );
    assert!(!out.contains("outdated"), "{out}");
    assert!(!out.contains("## Generated text"), "{out}");
    assert!(!docs.get_feature("pay-invoice").contains("Edit:"));
    assert!(!docs.get_domain("billing").contains("Edit:"));
}

// spec: MCP-16 (internal: the tool answer is checked without a JSON-RPC session)
#[test]
fn the_answer_of_an_outdated_edit_says_why_and_ends_with_the_generated_text() {
    let dir = repo("Ada");
    let fields = Fields {
        name: Some("Settle an invoice".into()),
        description: Some("Pays what is due.".into()),
        ..Fields::default()
    };
    let (_, edit) = edited(
        docs().with_source_access(dir.path()),
        dir.path(),
        &Section::new(SectionKind::Feature, "pay-invoice"),
        fields,
    );
    let rerun = Docs::new(
        vec![domain("billing", "Billing", "Invoices and payments")],
        vec![feature(
            "pay-invoice",
            "Pay an invoice",
            "Settle\nwhat is due by card",
        )],
        Vec::new(),
        &Glossary::default(),
        &[],
    )
    .with_source_access(dir.path())
    .with_edits(edits::load(dir.path()).unwrap());

    let out = rerun.get_feature("pay-invoice");

    assert!(out.starts_with("# Settle an invoice"), "{out}");
    assert!(
        out.contains(&format!(
            "\nEdit: human edit of the name and the description (Ada, {}), outdated: section \
             rewritten since the edit\n",
            edit.edited_at
        )),
        "{out}"
    );
    assert!(
        out.ends_with(
            "\n## Generated text\n\nThe edit is outdated: the last run generated, in place of the \
             edited fields:\n\n### Name\n\nPay an invoice\n\n### Description\n\nSettle\nwhat is \
             due by card\n"
        ),
        "{out}"
    );
}
