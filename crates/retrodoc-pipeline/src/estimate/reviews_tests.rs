use retrodoc_core::artifacts::estimate::RunEstimate;

use super::*;
use crate::corrections::{answered_key, Verdict};
use crate::estimate::estimate;

/// The passes a review asks, with their cost per unit: `domains` 1 call a run, `features` 1 to 2 calls a domain,
/// `use-cases` 1 call a feature, `rules` 3 calls a feature, `confidence` 0.5 call a use case; `reconcile` scales
/// with the files only.
const TABLE_YAML: &str = r#"
runs:
- {"application":"a","commit":"c","models":["m1"],"finished_at":"t"}
passes:
- pass: "domains"
  fixed: true
  per_unit: {"calls":{"min":1.0,"max":1.0},"prompt_tokens":{"min":100.0,"max":200.0},"completion_tokens":{"min":10.0,"max":20.0}}
  per_source_file: null
  applications: []
- pass: "features"
  fixed: false
  per_unit: {"calls":{"min":1.0,"max":2.0},"prompt_tokens":{"min":100.0,"max":200.0},"completion_tokens":{"min":10.0,"max":20.0}}
  per_source_file: {"calls":{"min":0.5,"max":1.0},"prompt_tokens":{"min":50.0,"max":100.0},"completion_tokens":{"min":5.0,"max":10.0}}
  applications: []
- pass: "use-cases"
  fixed: false
  per_unit: {"calls":{"min":1.0,"max":1.0},"prompt_tokens":{"min":1000.0,"max":2000.0},"completion_tokens":{"min":100.0,"max":200.0}}
  per_source_file: {"calls":{"min":1.0,"max":1.0},"prompt_tokens":{"min":1000.0,"max":2000.0},"completion_tokens":{"min":100.0,"max":200.0}}
  applications: []
- pass: "rules"
  fixed: false
  per_unit: {"calls":{"min":3.0,"max":3.0},"prompt_tokens":{"min":300.0,"max":600.0},"completion_tokens":{"min":30.0,"max":60.0}}
  per_source_file: {"calls":{"min":3.0,"max":3.0},"prompt_tokens":{"min":300.0,"max":600.0},"completion_tokens":{"min":30.0,"max":60.0}}
  applications: []
- pass: "confidence"
  fixed: false
  per_unit: {"calls":{"min":0.5,"max":0.5},"prompt_tokens":{"min":50.0,"max":50.0},"completion_tokens":{"min":5.0,"max":5.0}}
  per_source_file: {"calls":{"min":0.5,"max":0.5},"prompt_tokens":{"min":50.0,"max":50.0},"completion_tokens":{"min":5.0,"max":5.0}}
  applications: []
- pass: "reconcile"
  fixed: false
  per_unit: {"calls":{"min":1.0,"max":1.0},"prompt_tokens":{"min":10.0,"max":10.0},"completion_tokens":{"min":1.0,"max":1.0}}
  per_source_file: {"calls":{"min":1.0,"max":1.0},"prompt_tokens":{"min":10.0,"max":10.0},"completion_tokens":{"min":1.0,"max":1.0}}
  applications: []
"#;

const PASSES: &[&str] = &[
    "domains",
    "features",
    "use-cases",
    "rules",
    "confidence",
    "reconcile",
];

fn wrong(slug: &str) -> Correction {
    Correction {
        slug: slug.to_string(),
        verdict: Some(Verdict::Wrong {
            comment: "Not so.".to_string(),
            reviewed_at: "t".to_string(),
        }),
        ..Correction::default()
    }
}

/// A feature marked wrong, which held the entry point `<slug>#index` when the corrections were read.
fn wrong_feature(slug: &str) -> Correction {
    Correction {
        entry_points: vec![format!("{slug}#index")],
        ..wrong(slug)
    }
}

fn validated(slug: &str) -> Correction {
    Correction {
        slug: slug.to_string(),
        verdict: Some(Verdict::Validated),
        ..Correction::default()
    }
}

/// Three use cases saved for every feature.
fn units(prints: &Fingerprints, corrections: &Corrections) -> ReviewedUnits {
    ReviewedUnits::of(prints, corrections, |_| 3)
}

fn rerun(reviewed: ReviewedUnits, changed_files: Option<usize>) -> RunEstimate {
    let inputs = EstimateInputs {
        passes: PASSES,
        source_files: 10,
        changed_files,
        reviewed,
        ..EstimateInputs::default()
    };
    estimate(&inputs, &RatioTable::parse(TABLE_YAML).unwrap())
}

fn calls(estimate: &RunEstimate) -> Vec<(&str, u64, u64)> {
    estimate
        .passes
        .iter()
        .map(|p| (p.pass.as_str(), p.calls.low, p.calls.high))
        .collect()
}

// spec: EST-12, EST-13 (internal: the units counted per pass; the CLI tests cover one review end to end)
#[test]
fn a_use_case_marked_wrong_asks_its_feature_and_cascades_into_its_rules_and_confidence() {
    let mut corrections = Corrections::default();
    corrections.add_use_case("pay", wrong("pay-online"));
    corrections.add_use_case("pay", wrong("pay-cash"));
    corrections.add_use_case("ship", validated("ship-order"));

    let reviewed = units(&Fingerprints::default(), &corrections);
    let estimate = rerun(reviewed, Some(0));

    assert_eq!(
        calls(&estimate),
        [
            ("domains", 0, 0),
            ("features", 0, 0),
            ("use-cases", 1, 1),
            ("rules", 3, 3),
            ("confidence", 1, 2),
            ("reconcile", 0, 0),
        ]
    );
    let use_cases = &estimate.passes[2];
    assert_eq!(use_cases.basis, Basis::Reviewed);
    assert_eq!(
        (use_cases.prompt_tokens.low, use_cases.prompt_tokens.high),
        (1000, 2000)
    );
}

// spec: EST-12, EST-13 (internal: the units counted per pass)
#[test]
fn features_marked_wrong_ask_one_call_per_domain_and_the_use_cases_of_each() {
    let mut corrections = Corrections::default();
    corrections.add_feature("orders", wrong_feature("checkout"));
    corrections.add_feature("orders", wrong_feature("refund"));
    // A use case of a reviewed feature asks nothing more than its feature.
    corrections.add_use_case("refund", wrong("refund-card"));

    let estimate = rerun(units(&Fingerprints::default(), &corrections), Some(0));

    assert_eq!(
        calls(&estimate),
        [
            ("domains", 0, 0),
            // One call at least, whatever the lowest yield of the table.
            ("features", 1, 2),
            ("use-cases", 2, 2),
            ("rules", 6, 6),
            ("confidence", 3, 3),
            ("reconcile", 0, 0),
        ]
    );
}

// spec: EST-12, EST-13 (internal: the units counted per pass)
#[test]
fn a_domain_marked_wrong_asks_the_domains_call_and_nothing_downstream() {
    let mut corrections = Corrections::default();
    corrections.add_domain(wrong("routing"));
    corrections.add_domain(wrong("billing"));
    corrections.add_domain(wrong(UNCATEGORIZED_SLUG));

    let estimate = rerun(units(&Fingerprints::default(), &corrections), Some(0));

    assert_eq!(
        calls(&estimate),
        [
            ("domains", 1, 1),
            ("features", 0, 0),
            ("use-cases", 0, 0),
            ("rules", 0, 0),
            ("confidence", 0, 0),
            ("reconcile", 0, 0),
        ]
    );
}

// spec: EST-12 (internal: the record of fingerprints.json)
#[test]
fn a_review_its_unit_answered_asks_nothing() {
    let mut corrections = Corrections::default();
    corrections.add_domain(wrong("routing"));
    corrections.add_feature("orders", wrong_feature("checkout"));
    corrections.add_use_case("pay", wrong("pay-online"));
    let review = |c: Correction| vec![c.wrong_key().unwrap()];
    let mut prints = Fingerprints::default();
    prints
        .answered
        .insert(answered_key(DOMAINS, "-"), review(wrong("routing")));
    prints.answered.insert(
        answered_key(FEATURES, "orders/slice-2"),
        review(wrong_feature("checkout")),
    );
    // A domain asked in one call answers under `<domain>/-`.
    corrections.add_feature("billing", wrong_feature("invoice"));
    prints.answered.insert(
        answered_key(FEATURES, "billing/-"),
        review(wrong_feature("invoice")),
    );
    prints.answered.insert(
        answered_key(USE_CASES, "billing/pay"),
        review(wrong("pay-online")),
    );

    assert_eq!(units(&prints, &corrections), ReviewedUnits::default());

    // Answered by a unit of another domain, the review is still pending (and `billing` never answered there).
    let mut elsewhere = Fingerprints::default();
    elsewhere.answered.insert(
        answered_key(FEATURES, "orders-archive/-"),
        review(wrong_feature("checkout")),
    );
    assert_eq!(units(&elsewhere, &corrections).feature_domains, 2);
}

// spec: EST-12 (internal: the units counted on top of the changed files)
#[test]
fn the_units_asked_add_to_the_changed_files_and_nothing_to_a_forced_run() {
    let mut corrections = Corrections::default();
    corrections.add_domain(wrong("routing"));
    corrections.add_use_case("pay", wrong("pay-online"));
    let reviewed = units(&Fingerprints::default(), &corrections);

    let changed = rerun(reviewed.clone(), Some(2));
    let forced = rerun(reviewed, None);

    // The domains call is made once a run, whatever asks it.
    assert_eq!(calls(&changed)[0], ("domains", 1, 1));
    assert_eq!(calls(&changed)[2], ("use-cases", 3, 3));
    assert_eq!(calls(&forced)[2], ("use-cases", 10, 10));
}

// spec: EST-12 (internal: a feature the pass can't place in a slice)
#[test]
fn a_feature_with_no_entry_point_asks_nothing() {
    let mut corrections = Corrections::default();
    corrections.add_feature("orders", wrong("checkout"));

    assert_eq!(
        units(&Fingerprints::default(), &corrections),
        ReviewedUnits::default()
    );
}

// spec: EST-12, EST-6 (internal: the units counted per pass)
#[test]
fn a_call_a_review_asks_is_certain_and_the_confidence_sample_still_adds() {
    let table = TABLE_YAML.replace(
        r#"per_unit: {"calls":{"min":1.0,"max":2.0}"#,
        r#"per_unit: {"calls":{"min":0.8,"max":2.0}"#,
    );
    let mut corrections = Corrections::default();
    corrections.add_feature("orders", wrong_feature("checkout"));
    let inputs = EstimateInputs {
        passes: PASSES,
        source_files: 10,
        changed_files: Some(0),
        reviewed: units(&Fingerprints::default(), &corrections),
        confidence_sample: Some(4),
        ..EstimateInputs::default()
    };

    let estimate = estimate(&inputs, &RatioTable::parse(&table).unwrap());

    assert_eq!(calls(&estimate)[1], ("features", 1, 2));
    assert_eq!(calls(&estimate)[4], ("confidence", 1, 6));
}
