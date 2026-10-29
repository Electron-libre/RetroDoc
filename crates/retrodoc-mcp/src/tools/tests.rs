use retrodoc_core::model::{ConfidenceScore, RuleKind, SourceRef};

use super::*;
use crate::corpus::fixtures::*;

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
    let mut features = docs().features.clone();
    features
        .iter_mut()
        .find(|f| f.slug == "pay-invoice")
        .unwrap()
        .back_office = true;
    let mut docs = docs();
    docs.features = features;
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
