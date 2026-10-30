use std::collections::BTreeMap;
use std::path::PathBuf;

use retrodoc_core::config::RulesConfig;
use retrodoc_core::model::{ConfidenceScore, Feature, RuleEvidence, RuleKind, UseCase};
use retrodoc_ingest::signals::{Signal, SignalKind};

use super::grounding::RawRules;
use super::*;
use crate::brief::Evidence;
use crate::entry_points::{EntryFile, EntryKind, EntryPoint, EntryPoints};
use crate::slices::CodeIndex;
use crate::testing::{killed, reading, FakeLlm};

const ROUTER: &str = "def add_order\n  return :refused if time >= 60\n  accept\nend\n";

fn feature() -> Feature {
    Feature {
        back_office: false,
        slug: "orders".into(),
        domain_slug: "delivery".into(),
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
        narrative_check: None,
    }
}

fn context(root: &Path) -> UseCaseContext {
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
        ..reading(root)
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
    build_rules(dir, &[feature()], &[use_case()], &context(dir), llm, 1)
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
    let prompt = &llm.prompts()[1];
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

const COMPACT: &str = r#"{"statement":"An order that would take an hour or more to deliver is refused.","kind":"limit","use_cases":["place-an-order"],"code":["F1:2-2"],"tests":[{"id":"T1","phrase":"refuses an order that takes 60 minutes"}],"docs":["D1"]}"#;

#[tokio::test]
async fn a_compact_answer_cites_a_file_by_its_number_and_the_entities_come_from_the_statement() {
    let dir = repo();
    let llm = FakeLlm::answering(answer(COMPACT));

    let rules = run(dir.path(), &llm).await;

    assert_eq!(rules.len(), 1);
    let rule = &rules[0];
    assert_eq!(rule.entities, ["Order"]);
    assert_eq!(rule.use_cases, ["place-an-order"]);
    assert!(matches!(
        &rule.evidence[0],
        RuleEvidence::Code { source }
            if source.path == "lib/router.rb"
                && source.start_line == Some(2)
                && source.end_line == Some(2)
    ));
    assert_eq!(rule.evidence.len(), 3);
    let prompt = &llm.prompts()[1];
    assert!(prompt.contains("=== [F1] lib/router.rb ==="), "{prompt}");
}

#[test]
fn a_citation_is_read_as_a_number_with_lines_a_path_with_lines_or_the_old_object() {
    let raw: RawRules = serde_json::from_str(
        r#"{"rules":[{"statement":"s","kind":"limit","code":["F2:10-24","F1:7","F3","lib/a.rb:3-4",
        {"path":"lib/b.rb","start_line":1,"end_line":2},5]}]}"#,
    )
    .unwrap();
    let cited: Vec<_> = raw.rules[0]
        .code
        .iter()
        .map(|c| (c.path.as_deref(), c.start_line, c.end_line))
        .collect();
    assert_eq!(
        cited,
        [
            (Some("F2"), Some(10), Some(24)),
            (Some("F1"), Some(7), None),
            (Some("F3"), None, None),
            (Some("lib/a.rb"), Some(3), Some(4)),
            (Some("lib/b.rb"), Some(1), Some(2)),
        ]
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

// spec: RER-1, RER-3
#[tokio::test]
async fn an_unchanged_feature_is_not_asked_again_but_a_new_model_or_code_is() {
    let dir = repo();
    let first = FakeLlm::answering(answer(LIMIT));
    let rules = run(dir.path(), &first).await;
    assert_eq!(first.calls(), 3);

    let second = FakeLlm::answering("not even json");
    assert_eq!(run(dir.path(), &second).await, rules);
    assert_eq!(second.calls(), 0);

    let other_model = FakeLlm::answering(answer("")).with_model("big");
    assert_eq!(run(dir.path(), &other_model).await, []);
    assert_eq!(other_model.calls(), 2);

    std::fs::write(
        dir.path().join("lib/router.rb"),
        format!("{ROUTER}# edit\n"),
    )
    .unwrap();
    let after_edit = FakeLlm::answering(answer("")).with_model("big");
    run(dir.path(), &after_edit).await;
    assert_eq!(after_edit.calls(), 2);
}

#[tokio::test]
async fn a_feature_without_rules_is_remembered_but_an_unparseable_answer_is_retried() {
    let dir = repo();
    let none = FakeLlm::answering(answer(""));
    assert_eq!(run(dir.path(), &none).await, []);
    assert_eq!(run(dir.path(), &none).await, []);
    assert_eq!(none.calls(), 2);

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
    let result = build_rules(
        dir.path(),
        &[feature()],
        &[use_case()],
        &context(dir.path()),
        &down,
        1,
    )
    .await;

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
            "use_cases":{"slug":"place-an-order"},
            "code":[{"path":"lib/router.rb"}],
            "tests":["T2",{"id":"T1","phrase":"refuses an order"},7],
            "docs":[{"id":"D1"},"D2",null]}]}"#,
    )
    .unwrap();
    let r = &raw.rules[0];
    assert_eq!(r.use_cases, ["place-an-order"]);
    assert_eq!(r.docs, ["D1", "D2"]);
    let tests: Vec<(&str, &str)> = r
        .tests
        .iter()
        .map(|t| (t.id.as_str(), t.phrase.as_str()))
        .collect();
    assert_eq!(tests, [("T2", ""), ("T1", "refuses an order")]);
}

// spec: RER-5
#[tokio::test]
async fn a_killed_run_keeps_the_rules_of_the_features_done_and_the_rerun_resumes() {
    let dir = repo();
    let second = Feature {
        slug: "second".to_string(),
        ..feature()
    };
    let features = [feature(), second];

    let stuck = FakeLlm::hanging_after(3, |_| answer(LIMIT));
    killed(
        build_rules(
            dir.path(),
            &features,
            &[use_case()],
            &context(dir.path()),
            &stuck,
            1,
        ),
        &stuck,
    )
    .await;
    let saved = load_rules(dir.path()).expect("the rules of the feature done are on disk");
    assert_eq!(saved.len(), 1);

    let healthy = FakeLlm::answering(answer(LIMIT));
    build_rules(
        dir.path(),
        &features,
        &[use_case()],
        &context(dir.path()),
        &healthy,
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        healthy.calls(),
        3,
        "only the second feature is asked (kind, rules, judgement)"
    );
}

const STORAGE: &str = r#"{"statement":"An order is stored by the customer id.","kind":"validation","code":[{"path":"lib/router.rb","start_line":1,"end_line":1}]}"#;

/// An LLM that states `rules` for the feature and answers `verdict` to the judgement.
fn judging(rules: &str, verdict: &'static str) -> FakeLlm {
    let rules = rules.to_string();
    FakeLlm::replying(move |_, request| {
        Ok(
            if request.messages[0].content == judge::JUDGE_SYSTEM_PROMPT {
                verdict.to_string()
            } else {
                answer(&rules)
            },
        )
    })
}

#[tokio::test]
async fn a_rule_a_business_reader_would_not_ask_about_is_dropped_and_the_others_keep_their_slug() {
    let dir = repo();
    let llm = judging(
        &format!("{STORAGE},{LIMIT}"),
        r#"{"rules":[{"n":1,"class":"storage"},{"n":2,"class":"business"}]}"#,
    );

    let rules = run(dir.path(), &llm).await;

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].slug, "orders-2");
    assert_eq!(llm.calls(), 3);
    let judged = &llm.prompts()[2];
    assert!(judged.contains("Place an order"), "{judged}");
    assert!(
        judged.contains("1. [validation, code alone] An order is stored by the customer id."),
        "{judged}"
    );
    assert!(
        judged.contains("2. [limit, code, test and doc]"),
        "{judged}"
    );
    assert_eq!(load_rules(dir.path()).unwrap(), rules);
}

#[tokio::test]
async fn an_unusable_verdict_item_costs_nothing_and_a_feature_without_rules_is_not_judged() {
    let dir = repo();
    let llm = judging(
        LIMIT,
        r#"{"rules":[{"n":2.0,"class":"screen"},{"n":"x","class":"screen"},{"n":99,"class":"test"},{"n":0,"class":"technical"},{"class":"storage"},{"n":1,"class":"odd"},{"n":"1"}]}"#,
    );
    assert_eq!(run(dir.path(), &llm).await.len(), 1);

    let dir = repo();
    let none = judging(
        "",
        r#"{"rules":[{"n":1,"class":"storage"},{"n":2,"class":"business"}]}"#,
    );
    assert_eq!(run(dir.path(), &none).await, []);
    assert_eq!(none.calls(), 2);
}

#[tokio::test]
async fn a_judged_feature_is_not_asked_again_and_an_unparseable_verdict_keeps_the_rules() {
    let dir = repo();
    let first = judging(
        &format!("{STORAGE},{LIMIT}"),
        r#"{"rules":[{"n":1,"class":"storage"},{"n":2,"class":"business"}]}"#,
    );
    let rules = run(dir.path(), &first).await;
    let second = FakeLlm::answering("not even json");
    assert_eq!(run(dir.path(), &second).await, rules);
    assert_eq!(second.calls(), 0);

    let dir = repo();
    let garbage = judging(LIMIT, "sorry");
    let rules = run(dir.path(), &garbage).await;
    assert_eq!(rules.len(), 1, "an unusable verdict keeps the rules");
    let calls = garbage.calls();
    assert_eq!(run(dir.path(), &garbage).await, rules);
    assert_eq!(garbage.calls(), calls, "and the feature is not asked again");
}

#[tokio::test]
async fn a_new_judgement_prompt_judges_the_saved_features_again() {
    let dir = repo();
    run(dir.path(), &judging(LIMIT, r#"{"rules":[]}"#)).await;
    let mut prints = Fingerprints::load(dir.path());
    for print in prints.rules.values_mut() {
        *print = "from an older judgement prompt".to_string();
    }
    prints.save(dir.path()).unwrap();
    let again = judging(LIMIT, r#"{"rules":[]}"#);
    run(dir.path(), &again).await;
    assert_eq!(again.calls(), 3);
}

fn wide() -> Feature {
    Feature {
        slug: "wide".into(),
        name: "Run the whole router".into(),
        source_paths: vec!["lib/router.rb".into(), "lib/extra.rb".into()],
        ..feature()
    }
}

fn two_features_repo() -> tempfile::TempDir {
    let dir = repo();
    std::fs::write(dir.path().join("lib/extra.rb"), "def helper; end\n").unwrap();
    dir
}

async fn ask(dir: &Path, llm: &FakeLlm, features: &[Feature]) -> Vec<BusinessRule> {
    build_rules(dir, features, &[use_case()], &context(dir), llm, 1)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_rule_two_features_state_is_published_once_and_comes_back_when_its_owner_drops_it() {
    let dir = two_features_repo();
    let features = [feature(), wide()];
    let first = FakeLlm::answering(answer(LIMIT));
    let published = ask(dir.path(), &first, &features).await;
    // The feature that reads the fewest files keeps it; both stated it.
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].slug, "orders-1");
    assert_eq!(load_rules(dir.path()).unwrap(), published);
    let stated: Vec<BusinessRule> = load_yaml(&Artifact::RulesStated.path(dir.path())).unwrap();
    assert_eq!(stated.len(), 2);

    let second = FakeLlm::answering("not even json");
    assert_eq!(ask(dir.path(), &second, &features).await, published);
    assert_eq!(second.calls(), 0);

    // Its owner no longer states it: only that feature is asked again, and the copy of the other
    // one is published.
    let mut changed = features.clone();
    changed[0].description = "A customer orders a meal, or not".into();
    let third = FakeLlm::answering(answer(""));
    let after = ask(dir.path(), &third, &changed).await;
    assert_eq!(third.calls(), 2);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].slug, "wide-1");
}

/// An LLM that says whether the feature is about tests, and states `LIMIT` otherwise.
fn knowing(about_tests: &'static str) -> FakeLlm {
    FakeLlm::replying(move |_, request| {
        Ok(
            if request.messages[0].content == judge::FEATURE_SYSTEM_PROMPT {
                about_tests.to_string()
            } else {
                answer(LIMIT)
            },
        )
    })
}

#[tokio::test]
async fn a_feature_that_describes_tests_has_no_rules_and_no_statement_call() {
    let dir = repo();
    let llm = knowing(r#"{"about_tests":true}"#);

    assert_eq!(run(dir.path(), &llm).await, []);

    assert_eq!(llm.calls(), 1, "only the question about the feature");
    let asked = &llm.prompts()[0];
    assert!(asked.contains("Place an order"), "{asked}");
    assert!(!asked.contains("return :refused"), "no code: {asked}");
    // Remembered like a feature without rules.
    let again = knowing(r#"{"about_tests":true}"#);
    assert_eq!(run(dir.path(), &again).await, []);
    assert_eq!(again.calls(), 0);
}

#[tokio::test]
async fn a_feature_about_something_else_or_an_unreadable_answer_goes_on_to_its_rules() {
    for reply in [
        r#"{"about_tests":false}"#,
        "sorry",
        r#"{"about_tests":"maybe"}"#,
    ] {
        let dir = repo();
        let llm = knowing(reply);
        assert_eq!(run(dir.path(), &llm).await.len(), 1, "{reply}");
        // An unreadable answer is asked once more before it counts as no.
        let asked = if reply == "sorry" { 4 } else { 3 };
        assert_eq!(llm.calls(), asked, "question, rules, judgement: {reply}");
    }
    let dir = repo();
    assert_eq!(
        run(dir.path(), &knowing(r#"{"about_tests":"true"}"#)).await,
        []
    );
}

#[tokio::test]
async fn a_feature_set_aside_as_tests_is_kept_with_its_reason_until_it_changes() {
    let dir = repo();
    run(dir.path(), &knowing(r#"{"about_tests":true}"#)).await;

    let aside = load_set_aside(dir.path()).expect("the features set aside are on disk");
    assert_eq!(aside.features.len(), 1);
    let set_aside = &aside.features[0];
    assert_eq!(set_aside.slug, "orders");
    assert_eq!(set_aside.name, "Place an order");
    assert_eq!(set_aside.description, "A customer orders a meal");
    assert!(set_aside.reason.contains("tested"), "{}", set_aside.reason);

    // Reused with the feature, no call.
    let again = FakeLlm::answering("not even json");
    run(dir.path(), &again).await;
    assert_eq!(again.calls(), 0);
    assert_eq!(load_set_aside(dir.path()).unwrap(), aside);

    // Asked again and no longer about tests: nothing is set aside.
    let mut changed = feature();
    changed.description = "A customer orders a meal, or not".into();
    build_rules(
        dir.path(),
        &[changed],
        &[use_case()],
        &context(dir.path()),
        &knowing(r#"{"about_tests":false}"#),
        1,
    )
    .await
    .unwrap();
    assert!(load_set_aside(dir.path()).unwrap().is_empty());
}

#[tokio::test]
async fn a_rule_dropped_by_the_judgement_is_kept_with_its_class() {
    let dir = repo();
    let llm = judging(
        &format!("{STORAGE},{LIMIT}"),
        r#"{"rules":[{"n":1,"class":"storage"},{"n":2,"class":"business"}]}"#,
    );
    run(dir.path(), &llm).await;

    let aside = load_set_aside(dir.path()).unwrap();
    assert_eq!(aside.features, []);
    assert_eq!(aside.rules.len(), 1);
    let dropped = &aside.rules[0];
    assert_eq!(dropped.slug, "orders-1");
    assert_eq!(dropped.feature, "orders");
    assert_eq!(dropped.statement, "An order is stored by the customer id.");
    assert_eq!(dropped.class, "storage");

    let again = FakeLlm::answering("not even json");
    run(dir.path(), &again).await;
    assert_eq!(again.calls(), 0);
    assert_eq!(load_set_aside(dir.path()).unwrap(), aside);
}

#[tokio::test]
async fn a_run_saved_before_the_set_aside_file_is_asked_again_once() {
    let dir = repo();
    run(dir.path(), &knowing(r#"{"about_tests":true}"#)).await;
    std::fs::remove_file(Artifact::RulesSetAside.path(dir.path())).unwrap();

    let again = knowing(r#"{"about_tests":true}"#);
    run(dir.path(), &again).await;

    assert_eq!(again.calls(), 1);
    assert_eq!(load_set_aside(dir.path()).unwrap().features.len(), 1);
}

/// The context of the tests, with the features and rules of `put_back` put back by hand.
fn putting_back(root: &Path, features: &[&str], rules: &[&str]) -> UseCaseContext {
    UseCaseContext {
        put_back: RulesConfig {
            keep_features: features.iter().map(ToString::to_string).collect(),
            keep_rules: rules.iter().map(ToString::to_string).collect(),
        },
        ..context(root)
    }
}

async fn ask_with(
    dir: &Path,
    llm: &FakeLlm,
    features: &[Feature],
    context: &UseCaseContext,
) -> Vec<BusinessRule> {
    build_rules(dir, features, &[use_case()], context, llm, 1)
        .await
        .unwrap()
}

// spec: RSA-1
#[tokio::test]
async fn a_feature_put_back_by_hand_is_not_asked_whether_it_is_about_tests() {
    let dir = repo();
    let llm = knowing(r#"{"about_tests":true}"#);

    let rules = ask_with(
        dir.path(),
        &llm,
        &[feature()],
        &putting_back(dir.path(), &["orders"], &[]),
    )
    .await;

    assert_eq!(rules.len(), 1);
    assert_eq!(llm.calls(), 2, "statement and judgement only");
    assert!(!llm
        .system_prompts()
        .iter()
        .any(|p| p == judge::FEATURE_SYSTEM_PROMPT));
    assert_eq!(load_set_aside(dir.path()).unwrap().features, []);
}

// spec: RSA-2
#[tokio::test]
async fn putting_a_feature_back_or_taking_it_off_the_list_redoes_that_feature_only() {
    let dir = two_features_repo();
    let features = [feature(), wide()];
    let first = knowing(r#"{"about_tests":true}"#);
    ask_with(dir.path(), &first, &features, &context(dir.path())).await;
    assert_eq!(first.calls(), 2);

    let listed = knowing(r#"{"about_tests":true}"#);
    let rules = ask_with(
        dir.path(),
        &listed,
        &features,
        &putting_back(dir.path(), &["orders"], &[]),
    )
    .await;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].slug, "orders-1");
    assert_eq!(listed.calls(), 2, "{:?}", listed.prompts());
    assert!(listed
        .prompts()
        .iter()
        .all(|p| !p.contains("Run the whole router")));

    let unlisted = knowing(r#"{"about_tests":true}"#);
    assert_eq!(
        ask_with(dir.path(), &unlisted, &features, &context(dir.path())).await,
        []
    );
    assert_eq!(unlisted.calls(), 1, "the question on `orders` only");
}

// spec: RSA-5
#[tokio::test]
async fn a_feature_put_back_that_does_not_exist_is_warned_about() {
    let (logs, _guard) = crate::testing::capture_logs();
    let dir = repo();
    let llm = knowing(r#"{"about_tests":false}"#);

    let rules = ask_with(
        dir.path(),
        &llm,
        &[feature()],
        &putting_back(dir.path(), &["ghost", "orders"], &[]),
    )
    .await;

    assert_eq!(rules.len(), 1);
    let logs = logs.text();
    let warnings: Vec<&str> = logs.lines().filter(|l| l.contains("WARN")).collect();
    assert_eq!(warnings.len(), 1, "{logs}");
    assert!(
        warnings[0].contains("ghost") && warnings[0].contains("keep_features"),
        "{logs}"
    );
}

/// A run where `orders-1` (`STORAGE`) is dropped as storage and `orders-2` (`LIMIT`) kept.
async fn dropping_storage(dir: &Path) {
    let llm = judging(
        &format!("{STORAGE},{LIMIT}"),
        r#"{"rules":[{"n":1,"class":"storage"},{"n":2,"class":"business"}]}"#,
    );
    run(dir, &llm).await;
}

// spec: RSA-3, RSA-4
#[tokio::test]
async fn a_rule_put_back_by_hand_is_published_with_its_evidence_without_a_call_until_unlisted() {
    let dir = repo();
    dropping_storage(dir.path()).await;
    let dropped = load_set_aside(dir.path()).unwrap().rules[0].clone();
    assert_eq!(dropped.slug, "orders-1");

    let listed = FakeLlm::answering("not even json");
    let rules = ask_with(
        dir.path(),
        &listed,
        &[feature()],
        &putting_back(dir.path(), &[], &["orders-1"]),
    )
    .await;
    assert_eq!(listed.calls(), 0);
    let slugs: Vec<&str> = rules.iter().map(|r| r.slug.as_str()).collect();
    assert_eq!(slugs, ["orders-1", "orders-2"]);
    assert_eq!(rules[0].statement, "An order is stored by the customer id.");
    assert_eq!(rules[0].evidence, dropped.rule.clone().unwrap().evidence);
    assert_eq!(load_rules(dir.path()).unwrap(), rules);
    // Still a decision of the LLM on record: the set-aside file keeps it.
    assert_eq!(
        load_set_aside(dir.path()).unwrap().rules,
        std::slice::from_ref(&dropped)
    );

    let unlisted = FakeLlm::answering("not even json");
    let rules = run(dir.path(), &unlisted).await;
    assert_eq!(unlisted.calls(), 0);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].slug, "orders-2");
    assert_eq!(load_set_aside(dir.path()).unwrap().rules, [dropped]);
}

// spec: RSA-5
#[tokio::test]
async fn a_rule_put_back_that_was_not_set_aside_is_warned_about_unless_it_is_published() {
    let (logs, _guard) = crate::testing::capture_logs();
    let dir = repo();
    dropping_storage(dir.path()).await;

    let rules = ask_with(
        dir.path(),
        &FakeLlm::answering("not even json"),
        &[feature()],
        &putting_back(dir.path(), &[], &["orders-9", "orders-2"]),
    )
    .await;

    assert_eq!(rules.len(), 1);
    let logs = logs.text();
    let warnings: Vec<&str> = logs.lines().filter(|l| l.contains("WARN")).collect();
    assert_eq!(warnings.len(), 1, "{logs}");
    assert!(
        warnings[0].contains("orders-9") && warnings[0].contains("keep_rules"),
        "{logs}"
    );
}

// spec: RSA-8
#[tokio::test]
async fn a_rule_put_back_from_an_older_set_aside_file_is_warned_about_and_not_made_up() {
    let (logs, _guard) = crate::testing::capture_logs();
    let dir = repo();
    dropping_storage(dir.path()).await;
    // As written before the dropped rule was kept whole.
    let path = Artifact::RulesSetAside.path(dir.path());
    let mut older = load_set_aside(dir.path()).unwrap();
    older.rules[0].rule = None;
    save_yaml(&path, &older).unwrap();

    let rules = ask_with(
        dir.path(),
        &FakeLlm::answering("not even json"),
        &[feature()],
        &putting_back(dir.path(), &[], &["orders-1"]),
    )
    .await;

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].slug, "orders-2");
    let logs = logs.text();
    let warnings: Vec<&str> = logs.lines().filter(|l| l.contains("WARN")).collect();
    assert_eq!(warnings.len(), 1, "{logs}");
    assert!(
        warnings[0].contains("orders-1") && warnings[0].contains("stated again"),
        "{logs}"
    );
}

#[tokio::test]
async fn a_rule_listed_twice_is_put_back_once() {
    let dir = repo();
    dropping_storage(dir.path()).await;

    let rules = ask_with(
        dir.path(),
        &FakeLlm::answering("not even json"),
        &[feature()],
        &putting_back(dir.path(), &[], &["orders-1", "orders-1"]),
    )
    .await;

    let slugs: Vec<&str> = rules.iter().map(|r| r.slug.as_str()).collect();
    assert_eq!(slugs, ["orders-1", "orders-2"]);
}

#[tokio::test]
async fn a_failing_run_keeps_the_rules_put_back_in_what_it_saves() {
    let dir = two_features_repo();
    let features = [feature(), wide()];
    let llm = judging(
        &format!("{STORAGE},{LIMIT}"),
        r#"{"rules":[{"n":1,"class":"storage"},{"n":2,"class":"business"}]}"#,
    );
    ask_with(dir.path(), &llm, &features, &context(dir.path())).await;
    let mut changed = features.clone();
    changed[1].description = "Runs everything, or not".into();

    let down = FakeLlm::failing_after(0, |_| String::new());
    let result = build_rules(
        dir.path(),
        &changed,
        &[use_case()],
        &putting_back(dir.path(), &[], &["orders-1"]),
        &down,
        1,
    )
    .await;

    assert!(result.is_err());
    let saved = load_rules(dir.path()).unwrap();
    assert!(saved.iter().any(|r| r.slug == "orders-1"), "{saved:?}");
}

#[tokio::test]
async fn a_feature_put_back_whose_answer_is_unreadable_is_no_longer_listed_as_set_aside() {
    let dir = repo();
    run(dir.path(), &knowing(r#"{"about_tests":true}"#)).await;
    assert_eq!(load_set_aside(dir.path()).unwrap().features.len(), 1);

    ask_with(
        dir.path(),
        &FakeLlm::answering("sorry"),
        &[feature()],
        &putting_back(dir.path(), &["orders"], &[]),
    )
    .await;

    assert_eq!(load_set_aside(dir.path()).unwrap().features, []);
}

#[test]
fn the_entities_of_a_rule_are_the_ones_its_statement_speaks_of_as_whole_words() {
    let allowed = ["Order", "Rider", "Delivery Slot", "Rid"];
    assert_eq!(
        grounding::mentioned(
            "Orders are refused, and a delivery slot is freed.",
            &allowed
        ),
        ["Order", "Delivery Slot"]
    );
    assert_eq!(
        grounding::mentioned(
            "Addresses and categories are checked.",
            &["Address", "Category"]
        ),
        ["Address", "Category"]
    );
    let none = grounding::mentioned("Preorders wait for a ride.", &allowed);
    assert_eq!(none.len(), 0, "{none:?}");
}

#[test]
fn a_slip_in_the_lines_of_a_numbered_file_costs_the_lines_not_the_file() {
    let raw: RawRules =
        serde_json::from_str(r#"{"rules":[{"code":["F2:L10-24","F2:10\u201324","lib/a.rb:x"]}]}"#)
            .unwrap();
    let cited: Vec<_> = raw.rules[0]
        .code
        .iter()
        .map(|c| (c.path.as_deref(), c.start_line))
        .collect();
    assert_eq!(
        cited,
        [
            (Some("F2"), None),
            (Some("F2"), None),
            (Some("lib/a.rb:x"), None)
        ]
    );
}

const EXTRA: &str = "def helper\n  return :refused if retries > 3\nend\n";

/// A feature that reads two files, each with an entry point.
fn two_file_context(root: &Path) -> UseCaseContext {
    let mut context = context(root);
    let entry = |name: &str| EntryPoint {
        kind: EntryKind::Job,
        name: name.into(),
        verb: "run".into(),
        resource: "job".into(),
        description: name.into(),
        outputs: Vec::new(),
    };
    context.entry_points.files.insert(
        PathBuf::from("lib/extra.rb"),
        EntryFile {
            content_hash: String::new(),
            entry_points: vec![entry("helper")],
        },
    );
    context.index = CodeIndex::new(["lib/router.rb", "lib/extra.rb"].map(Path::new));
    context
}

fn extra_rule(path_ref: &str) -> String {
    format!(
        r#"{{"statement":"A delivery is abandoned after three retries.","kind":"limit","code":["{path_ref}:2-2"]}}"#
    )
}

/// Answers each prompt with the rule of the file it shows, and cuts the prompts that show both.
fn answering_per_file() -> FakeLlm {
    FakeLlm::replying(|_, request| {
        let prompt = &request.messages[1].content;
        let mut rules = Vec::new();
        if prompt.contains("=== [F1] lib/router.rb") || prompt.contains("=== [F2] lib/router.rb") {
            rules.push(LIMIT_COMPACT.to_string());
        }
        if prompt.contains("lib/extra.rb ===") {
            let number = if prompt.contains("=== [F1] lib/extra.rb") {
                "F1"
            } else {
                "F2"
            };
            rules.push(extra_rule(number));
        }
        Ok(answer(&rules.join(",")))
    })
    .truncating_prompts_over(0)
}

const LIMIT_COMPACT: &str = r#"{"statement":"An order that would take an hour or more to deliver is refused.","kind":"limit","code":["F1:2-2"]}"#;

#[tokio::test]
async fn an_answer_cut_by_the_completion_limit_is_asked_again_on_halves_of_the_files_and_merged() {
    let dir = two_features_repo();
    std::fs::write(dir.path().join("lib/extra.rb"), EXTRA).unwrap();
    let context = two_file_context(dir.path());
    // Only the prompt with both files is cut: the halves are shorter.
    let probe_dir = two_features_repo();
    std::fs::write(probe_dir.path().join("lib/extra.rb"), EXTRA).unwrap();
    let probe = FakeLlm::answering(answer(""));
    build_rules(
        probe_dir.path(),
        &[wide()],
        &[use_case()],
        &context,
        &probe,
        1,
    )
    .await
    .unwrap();
    let both = probe.prompts().iter().map(String::len).max().unwrap();
    assert!(
        probe.prompts()[1].contains("lib/extra.rb"),
        "{:?}",
        probe.prompts()
    );

    let llm = answering_per_file().truncating_prompts_over(both - 1);
    let rules = build_rules(dir.path(), &[wide()], &[use_case()], &context, &llm, 1)
        .await
        .unwrap();

    // 1 kind question, the cut call, the two halves, then the judgement of the merged rules.
    assert_eq!(llm.calls(), 5, "{:?}", llm.prompts());
    let slugs: Vec<&str> = rules.iter().map(|r| r.slug.as_str()).collect();
    assert_eq!(slugs, ["wide-1", "wide-2"]);
    // The files go in the order of the prompt (`extra` before `router`), the first half first.
    assert!(rules[0].statement.starts_with("A delivery"), "{rules:?}");
    assert!(rules[1].statement.starts_with("An order"), "{rules:?}");
}

#[tokio::test]
async fn a_single_file_that_is_still_cut_leaves_the_feature_to_the_next_run() {
    let dir = repo();
    let llm = FakeLlm::answering(answer(COMPACT)).truncating_prompts_over(0);
    let rules = run(dir.path(), &llm).await;
    assert_eq!(rules.len(), 0);
    // Not remembered as "no rules": the next run asks again.
    let again = FakeLlm::answering(answer(COMPACT));
    assert_eq!(run(dir.path(), &again).await.len(), 1);
}

#[test]
fn a_statement_two_parts_make_is_kept_once_with_the_better_backed_copy_and_the_rules_renumbered() {
    let rule = |slug: &str, statement: &str, confidence: f32| BusinessRule {
        slug: slug.into(),
        statement: statement.into(),
        kind: RuleKind::Limit,
        entities: Vec::new(),
        use_cases: Vec::new(),
        evidence: Vec::new(),
        confidence: Some(ConfidenceScore::new(confidence, None)),
    };
    let merged = split::merged(
        vec![
            rule("f-1", "A retry limit applies.", 0.4),
            rule("f-2", "Orders are refused.", 0.4),
            rule("f-1", "a retry  limit applies.", 0.7),
        ],
        "f",
    );
    let seen: Vec<(&str, f32)> = merged
        .iter()
        .map(|r| (r.slug.as_str(), r.confidence.as_ref().unwrap().value))
        .collect();
    assert_eq!(seen, [("f-1", 0.7), ("f-2", 0.4)]);
}

/// The saved prints must not change with a refactor: another print redoes every user's next run.
#[tokio::test]
async fn the_print_of_a_feature_stays_the_same() {
    let dir = repo();
    let llm = FakeLlm::answering(answer(LIMIT));
    let context = UseCaseContext {
        brief: crate::testing::brief("Delivers meals."),
        ..context(dir.path())
    };
    build_rules(dir.path(), &[feature()], &[use_case()], &context, &llm, 1)
        .await
        .unwrap();
    let prints = crate::fingerprints::Fingerprints::load(dir.path()).rules;
    assert_eq!(
        prints.into_iter().collect::<Vec<_>>(),
        vec![(
            "delivery/orders".to_string(),
            "39d25ac7c67ab0fde88e782117c00d16840309a976fcf90dd98dc572683edcc3".to_string()
        )]
    );
}
