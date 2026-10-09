use super::*;

use crate::testing::FakeLlm;
use retrodoc_core::model::{Actor, ActorKind, SourceRef, Step};

fn step(order: u32, path: Option<&str>) -> Step {
    Step {
        order,
        description: "d".to_string(),
        actor: Actor {
            name: "API".to_string(),
            kind: ActorKind::System,
        },
        action: "does".to_string(),
        source_refs: path
            .map(|p| SourceRef {
                path: p.to_string(),
                start_line: None,
                end_line: None,
            })
            .into_iter()
            .collect(),
    }
}

fn use_case(feature: &str, steps: Vec<Step>) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: "u".to_string(),
        feature_slug: feature.to_string(),
        name: "U".to_string(),
        description: "d".to_string(),
        steps,
        diagram_mermaid: None,
        confidence: None,
    }
}

fn feature(slug: &str) -> Feature {
    Feature {
        slug: slug.to_string(),
        domain_slug: "billing".to_string(),
        sub_domain_slug: None,
        name: slug.to_string(),
        description: "d".to_string(),
        source_paths: vec![],
        confidence: None,
    }
}

#[tokio::test]
async fn scores_steps_caps_ungrounded_ones_and_aggregates_to_the_feature() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::answering(
        r#"{"steps":[
              {"order":1,"verdict":"supported"},
              {"order":2,"verdict":"supported"},
              {"order":3,"verdict":"unsupported","rationale":"nothing about emails"}]}"#,
    );
    let mut features = vec![feature("f"), feature("empty")];
    let mut use_cases = vec![use_case(
        "f",
        vec![step(1, Some("a.rs")), step(2, None), step(3, Some("a.rs"))],
    )];

    score_confidence(dir.path(), &mut features, &mut use_cases, &provider, None)
        .await
        .unwrap();

    // (1.0 + 0.25 + 0.0) / 3
    let uc = use_cases[0].confidence.as_ref().unwrap();
    assert!((uc.value - 1.25 / 3.0).abs() < 1e-6);
    let why = uc.rationale.as_deref().unwrap();
    assert!(why.contains("step 2: cites no code"));
    assert!(why.contains("step 3: nothing about emails"));
    assert_eq!(features[0].confidence.as_ref().unwrap().value, uc.value);
    assert_eq!(features[1].confidence.as_ref().unwrap().value, 0.0);

    let saved = crate::load_use_cases(dir.path()).unwrap();
    assert!(saved[0].confidence.is_some());
    assert!(crate::load_features(dir.path()).unwrap()[0]
        .confidence
        .is_some());
}

#[tokio::test]
async fn no_readable_code_scores_zero_and_bad_answer_stays_unscored() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let mut features = vec![feature("f")];
    let mut use_cases = vec![
        use_case("f", vec![step(1, None)]),
        use_case("f", vec![step(1, Some("a.rs"))]),
    ];

    score_confidence(
        dir.path(),
        &mut features,
        &mut use_cases,
        &FakeLlm::answering("nope"),
        None,
    )
    .await
    .unwrap();

    assert_eq!(use_cases[0].confidence.as_ref().unwrap().value, 0.0);
    assert!(use_cases[1].confidence.is_none());
    // Feature mean only over the scored use case.
    let fc = features[0].confidence.as_ref().unwrap();
    assert_eq!(fc.value, 0.0);
    assert_eq!(fc.rationale.as_deref(), Some("1 of 2 use case(s) scored"));
}

#[tokio::test]
async fn a_sample_scores_only_that_many_use_cases() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::answering(r#"{"steps":[{"order":1,"verdict":"supported"}]}"#);
    let mut features = vec![feature("f")];
    let mut use_cases: Vec<UseCase> = (0..6)
        .map(|_| use_case("f", vec![step(1, Some("a.rs"))]))
        .collect();

    score_confidence(
        dir.path(),
        &mut features,
        &mut use_cases,
        &provider,
        Some(2),
    )
    .await
    .unwrap();

    assert_eq!(
        use_cases.iter().filter(|u| u.confidence.is_some()).count(),
        2
    );
    let fc = features[0].confidence.as_ref().unwrap();
    assert_eq!(fc.rationale.as_deref(), Some("2 of 6 use case(s) scored"));
}

#[tokio::test]
async fn use_cases_of_a_feature_share_a_request_and_missing_ones_fall_back() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::replying(|_, request| {
        // Batched requests get a verdict for `u1` and `u2` only, any
        // other a lone verdict list.
        Ok(
            if request.messages[1].content.starts_with("Several use cases") {
                r#"{"use_cases":[
                  {"slug":"u1","steps":[{"order":1,"verdict":"supported"}]},
                  {"slug":"u2","steps":[{"order":1,"verdict":"partial","rationale":"vague"}]}]}"#
            } else {
                r#"{"steps":[{"order":1,"verdict":"unsupported","rationale":"none"}]}"#
            }
            .to_string(),
        )
    });
    let mut features = vec![feature("f")];
    let mut use_cases: Vec<UseCase> = ["u1", "u2", "u3"]
        .iter()
        .map(|slug| {
            let mut u = use_case("f", vec![step(1, Some("a.rs"))]);
            u.slug = (*slug).to_string();
            u
        })
        .collect();

    score_confidence(dir.path(), &mut features, &mut use_cases, &provider, None)
        .await
        .unwrap();

    let value = |i: usize| use_cases[i].confidence.as_ref().unwrap().value;
    assert_eq!((value(0), value(1), value(2)), (1.0, 0.5, 0.0));
    // One batched request, one fallback for u3.
    assert_eq!(provider.calls(), 2);
}

#[tokio::test]
async fn a_long_cited_file_is_shown_around_the_cited_lines() {
    use std::fmt::Write as _;

    let dir = tempfile::tempdir().unwrap();
    let mut code = String::new();
    for i in 1..=60 {
        let _ = writeln!(code, "def action_{i}");
        for step in 1..=8 {
            let _ = writeln!(code, "  work_{i}_{step}");
        }
        let _ = writeln!(code, "end\n");
    }
    std::fs::write(dir.path().join("big.rb"), &code).unwrap();
    // `def action_50` is at line 1 + 49 * 11.
    let line = 1 + 49 * 11;
    let mut cited = step(1, Some("big.rb"));
    cited.source_refs[0].start_line = Some(line);
    cited.source_refs[0].end_line = Some(line + 5);
    let mut features = vec![feature("f")];
    let mut use_cases = vec![use_case("f", vec![cited])];
    let spy = FakeLlm::answering(r#"{"steps":[{"order":1,"verdict":"supported"}]}"#);

    score_confidence(dir.path(), &mut features, &mut use_cases, &spy, None)
        .await
        .unwrap();

    let prompts = spy.prompts();
    assert!(prompts[0].contains("def action_50"), "cited code is shown");
    assert!(prompts[0].contains("omitted)"));
    assert!(!prompts[0].contains("def action_30"));
}

#[tokio::test]
async fn another_model_scores_the_scored_use_cases_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let answer = r#"{"steps":[{"order":1,"verdict":"supported"}]}"#;
    let mut features = vec![feature("f")];
    let mut use_cases = vec![use_case("f", vec![step(1, Some("a.rs"))])];
    let mut calls = Vec::new();
    for model in ["local", "local", "strong"] {
        let provider = FakeLlm::answering(answer).with_model(model);
        score_confidence(dir.path(), &mut features, &mut use_cases, &provider, None)
            .await
            .unwrap();
        calls.push(provider.calls());
    }
    assert_eq!(calls, [1, 0, 1]);
    assert!(use_cases[0].confidence.is_some());
}
