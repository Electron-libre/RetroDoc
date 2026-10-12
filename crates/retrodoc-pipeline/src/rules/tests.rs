use std::collections::BTreeMap;
use std::path::PathBuf;

use retrodoc_core::model::{Feature, RuleEvidence, RuleKind, UseCase};
use retrodoc_ingest::signals::{Signal, SignalKind};

use super::*;
use crate::brief::Evidence;
use crate::entry_points::{EntryFile, EntryKind, EntryPoint, EntryPoints};
use crate::slices::CodeIndex;
use crate::testing::FakeLlm;

const ROUTER: &str = "def add_order\n  return :refused if time >= 60\n  accept\nend\n";

fn feature() -> Feature {
    Feature {
        slug: "orders".into(),
        domain_slug: "delivery".into(),
        sub_domain_slug: None,
        name: "Place an order".into(),
        description: "A customer orders a meal".into(),
        source_paths: vec!["lib/router.rb".into()],
        entry_points: Vec::new(),
        confidence: None,
    }
}

fn use_case() -> UseCase {
    UseCase {
        slug: "place-an-order".into(),
        feature_slug: "orders".into(),
        name: "Place an order".into(),
        description: String::new(),
        steps: Vec::new(),
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        diagram_mermaid: None,
        confidence: None,
    }
}

fn context() -> UseCaseContext {
    let signal = |kind, origin: &str, text: &str| Signal {
        kind,
        origin: origin.into(),
        text: text.into(),
    };
    UseCaseContext {
        entry_points: EntryPoints {
            files: BTreeMap::from([(
                PathBuf::from("lib/router.rb"),
                EntryFile {
                    content_hash: String::new(),
                    entry_points: vec![EntryPoint {
                        kind: EntryKind::Job,
                        name: "add_order".into(),
                        verb: "add".into(),
                        resource: "order".into(),
                        description: "Place an order".into(),
                        outputs: Vec::new(),
                    }],
                },
            )]),
        },
        index: CodeIndex::new([Path::new("lib/router.rb")]),
        vocabulary: vec!["Order".into(), "Rider".into()],
        rule_evidence: Evidence::new(&[
            signal(
                SignalKind::TestDescriptions,
                "spec/router_spec.rb",
                "refuses an order that takes 60 minutes; delights the customer",
            ),
            signal(
                SignalKind::DocSection,
                "README.md#Limits",
                "An order is refused when delivery takes an hour.",
            ),
        ]),
        ..UseCaseContext::default()
    }
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    std::fs::write(dir.path().join("lib/router.rb"), ROUTER).unwrap();
    dir
}

fn answer(rules: &str) -> String {
    format!(r#"{{"rules":[{rules}]}}"#)
}

const LIMIT: &str = r#"{"statement":"An order that would take an hour or more to deliver is refused.","kind":"limit","entities":["order","Ghost"],"use_cases":["place-an-order","nope"],"code":[{"path":"lib/router.rb","start_line":2,"end_line":2}],"tests":[{"id":"T1","phrase":"refuses an order that takes 60 minutes"}],"docs":["D1"]}"#;

async fn run(dir: &Path, llm: &FakeLlm) -> Vec<BusinessRule> {
    build_rules(dir, &[feature()], &[use_case()], &context(), llm)
        .await
        .unwrap()
}

#[tokio::test]
async fn states_a_rule_with_its_evidence_and_a_confidence() {
    let dir = repo();
    let llm = FakeLlm::answering(answer(LIMIT));

    let rules = run(dir.path(), &llm).await;

    assert_eq!(rules.len(), 1);
    let rule = &rules[0];
    assert_eq!(rule.slug, "orders-1");
    assert_eq!(rule.kind, RuleKind::Limit);
    // Only the entities and use cases of the lists, as they are spelled there.
    assert_eq!(rule.entities, ["Order"]);
    assert_eq!(rule.use_cases, ["place-an-order"]);
    assert_eq!(rule.evidence.len(), 3);
    assert!(matches!(
        &rule.evidence[1],
        RuleEvidence::Test { path, .. } if path == "spec/router_spec.rb"
    ));
    assert!(matches!(
        &rule.evidence[2],
        RuleEvidence::Doc { path, heading } if path == "README.md" && heading == "Limits"
    ));
    assert!((rule.confidence.as_ref().unwrap().value - 0.85).abs() < 1e-6);
    assert_eq!(load_rules(dir.path()).unwrap(), rules);
}

#[tokio::test]
async fn the_prompt_numbers_the_code_and_gives_an_id_to_each_evidence() {
    let dir = repo();
    let llm = FakeLlm::answering(answer(LIMIT));
    run(dir.path(), &llm).await;
    let prompt = &llm.prompts()[0];
    assert!(prompt.contains("[T1] refuses an order"), "{prompt}");
    assert!(prompt.contains("[D1] An order is refused"), "{prompt}");
    assert!(prompt.contains("   2 |   return :refused"), "{prompt}");
    assert!(prompt.contains("Entities: Order"), "{prompt}");
    assert!(
        prompt.contains("- place-an-order: Place an order"),
        "{prompt}"
    );
    assert!(
        !prompt.contains("spec/router_spec.rb"),
        "evidence is cited by id, never by path: {prompt}"
    );
}

#[tokio::test]
async fn code_alone_is_a_hypothesis() {
    let dir = repo();
    let llm = FakeLlm::answering(answer(
        r#"{"statement":"Orders are refused after an hour.","kind":"limit","code":[{"path":"lib/router.rb","start_line":2,"end_line":2}]}"#,
    ));
    let rules = run(dir.path(), &llm).await;
    assert!((rules[0].confidence.as_ref().unwrap().value - 0.4).abs() < 1e-6);
    assert!(!rules[0].has_corroboration());
}

#[tokio::test]
async fn a_rule_the_code_does_not_support_is_dropped() {
    let dir = repo();
    let rule = |statement: &str, kind: &str, code: &str| {
        format!(r#"{{"statement":"{statement}","kind":"{kind}","code":[{code}]}}"#)
    };
    let ok = r#"{"path":"lib/router.rb","start_line":1,"end_line":4}"#;
    let answer = answer(
        &[
            rule(
                "Lines beyond the file.",
                "limit",
                r#"{"path":"lib/router.rb","start_line":3,"end_line":40}"#,
            ),
            rule(
                "A file nobody showed.",
                "limit",
                r#"{"path":"lib/other.rb","start_line":1,"end_line":2}"#,
            ),
            rule("No citation.", "limit", ""),
            rule("Unknown kind.", "mystery", ok),
            rule("  ", "limit", ok),
            rule("Kept.", "State-Transition", ok),
            rule("kept.", "limit", ok),
        ]
        .join(","),
    );
    let rules = run(dir.path(), &FakeLlm::answering(answer)).await;
    let statements: Vec<&str> = rules.iter().map(|r| r.statement.as_str()).collect();
    assert_eq!(statements, ["Kept."]);
    assert_eq!(rules[0].kind, RuleKind::StateTransition);
}

#[tokio::test]
async fn a_quote_that_is_not_in_the_test_or_an_unknown_id_is_no_evidence() {
    let dir = repo();
    let llm = FakeLlm::answering(answer(
        r#"{"statement":"Orders are refused after an hour.","kind":"limit","code":[{"path":"lib/router.rb"}],"tests":[{"id":"T1","phrase":"something invented"},{"id":"T9","phrase":"delights the customer"}],"docs":["D7"]}"#,
    ));
    let rules = run(dir.path(), &llm).await;
    assert_eq!(rules[0].evidence.len(), 1, "{:?}", rules[0].evidence);
}

#[tokio::test]
async fn an_unchanged_feature_is_not_asked_again_but_a_new_model_or_code_is() {
    let dir = repo();
    let first = FakeLlm::answering(answer(LIMIT));
    let rules = run(dir.path(), &first).await;
    assert_eq!(first.calls(), 1);

    let second = FakeLlm::answering("not even json");
    assert_eq!(run(dir.path(), &second).await, rules);
    assert_eq!(second.calls(), 0);

    let other_model = FakeLlm::answering(answer("")).with_model("big");
    assert_eq!(run(dir.path(), &other_model).await, []);
    assert_eq!(other_model.calls(), 1);

    std::fs::write(
        dir.path().join("lib/router.rb"),
        format!("{ROUTER}# edit\n"),
    )
    .unwrap();
    let after_edit = FakeLlm::answering(answer("")).with_model("big");
    run(dir.path(), &after_edit).await;
    assert_eq!(after_edit.calls(), 1);
}

#[tokio::test]
async fn a_feature_without_rules_is_remembered_but_an_unparseable_answer_is_retried() {
    let dir = repo();
    let none = FakeLlm::answering(answer(""));
    assert_eq!(run(dir.path(), &none).await, []);
    assert_eq!(run(dir.path(), &none).await, []);
    assert_eq!(none.calls(), 1);

    let dir = repo();
    let garbage = FakeLlm::answering("sorry");
    assert_eq!(run(dir.path(), &garbage).await, []);
    let calls = garbage.calls();
    run(dir.path(), &garbage).await;
    assert!(garbage.calls() > calls);
}

#[tokio::test]
async fn a_failing_call_keeps_the_previous_rules_of_the_features_not_reached() {
    let dir = repo();
    run(dir.path(), &FakeLlm::answering(answer(LIMIT))).await;
    std::fs::write(
        dir.path().join("lib/router.rb"),
        format!("{ROUTER}# edit\n"),
    )
    .unwrap();

    let down = FakeLlm::failing_after(0, |_| String::new());
    let result = build_rules(dir.path(), &[feature()], &[use_case()], &context(), &down).await;

    assert!(result.is_err());
    assert_eq!(load_rules(dir.path()).unwrap().len(), 1);
}

#[tokio::test]
async fn an_unparseable_answer_keeps_the_saved_rules_of_the_feature() {
    let dir = repo();
    let rules = run(dir.path(), &FakeLlm::answering(answer(LIMIT))).await;
    std::fs::write(
        dir.path().join("lib/router.rb"),
        format!("{ROUTER}# edit\n"),
    )
    .unwrap();

    assert_eq!(run(dir.path(), &FakeLlm::answering("sorry")).await, rules);
    assert_eq!(load_rules(dir.path()).unwrap(), rules);
}

#[tokio::test]
async fn a_repeated_statement_is_kept_once_with_the_copy_that_has_valid_code() {
    let dir = repo();
    let copy = |lines: &str| {
        format!(
            r#"{{"statement":"Orders are refused after an hour.","kind":"limit","code":[{{"path":"lib/router.rb",{lines}}}]}}"#
        )
    };
    let llm = FakeLlm::answering(answer(&format!(
        "{},{},{}",
        copy(r#""start_line":3,"end_line":99"#),
        copy(r#""start_line":2,"end_line":2"#),
        copy(r#""start_line":1,"end_line":1"#),
    )));
    let rules = run(dir.path(), &llm).await;
    assert_eq!(rules.len(), 1);
    assert!(matches!(
        &rules[0].evidence[0],
        RuleEvidence::Code { source } if source.start_line == Some(2)
    ));
}

#[test]
fn a_hosted_model_s_odd_shapes_cost_the_element_not_the_answer() {
    let raw: grounding::RawRules = serde_json::from_str(
        r#"{"rules":[{"statement":"s","kind":"limit",
            "entities":[{"name":"Order"},"Rider",3],
            "use_cases":{"slug":"place-an-order"},
            "code":[{"path":"lib/router.rb"}],
            "tests":["T2",{"id":"T1","phrase":"refuses an order"},7],
            "docs":[{"id":"D1"},"D2",null]}]}"#,
    )
    .unwrap();
    let r = &raw.rules[0];
    assert_eq!(r.entities, ["Order", "Rider"]);
    assert_eq!(r.use_cases, ["place-an-order"]);
    assert_eq!(r.docs, ["D1", "D2"]);
    let tests: Vec<(&str, &str)> = r
        .tests
        .iter()
        .map(|t| (t.id.as_str(), t.phrase.as_str()))
        .collect();
    assert_eq!(tests, [("T2", ""), ("T1", "refuses an order")]);
}
