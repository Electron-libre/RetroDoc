use super::*;

use crate::testing::{killed, FakeLlm};
use crate::use_cases::load_use_cases;
use retrodoc_core::model::{Actor, ActorKind, SourceRef, Step};
use std::path::Path;

fn step(order: u32, path: Option<&str>) -> Step {
    Step {
        order,
        description: "d".to_string(),
        actor: Actor {
            name: "API".to_string(),
            kind: ActorKind::System,
        },
        // Names something the test files do not contain: the checks leave it to the model.
        action: "calls missing_fn".to_string(),
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
        back_office: false,
        slug: slug.to_string(),
        domain_slug: "billing".to_string(),
        sub_domain_slug: None,
        name: slug.to_string(),
        description: "d".to_string(),
        source_paths: vec![],
        entry_points: Vec::new(),
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

    score_confidence(
        dir.path(),
        &mut features,
        &mut use_cases,
        &provider,
        None,
        1,
    )
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
        1,
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

/// A step the checks settle alone: it names `a` and cites the line that holds it.
fn passing_step() -> Step {
    let mut s = step(1, Some("a.rs"));
    s.action = "calls `a`".to_string();
    s.source_refs[0].start_line = Some(1);
    s.source_refs[0].end_line = Some(1);
    s
}

#[tokio::test]
async fn steps_that_pass_the_checks_are_scored_without_the_model() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::answering("unused");
    let mut features = vec![feature("f")];
    let mut broken = step(2, Some("a.rs"));
    broken.source_refs[0].start_line = Some(40);
    let mut use_cases = vec![
        use_case("f", vec![passing_step()]),
        use_case("f", vec![passing_step(), broken, step(3, Some("a.rs"))]),
    ];
    use_cases[1].steps[2].action = "calls `a`".to_string();

    score_confidence(
        dir.path(),
        &mut features,
        &mut use_cases,
        &provider,
        None,
        1,
    )
    .await
    .unwrap();

    assert_eq!(provider.calls(), 0);
    assert_eq!(use_cases[0].confidence.as_ref().unwrap().value, 1.0);
    let mixed = use_cases[1].confidence.as_ref().unwrap();
    // Passing step 1, broken step 2 (no line 40), step 3 passes: 2/3.
    assert!((mixed.value - 2.0 / 3.0).abs() < 1e-6);
    assert!(mixed.rationale.as_deref().unwrap().contains("step 2"));
}

#[tokio::test]
async fn only_the_doubtful_steps_are_sent_to_the_model() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::answering(r#"{"steps":[{"order":2,"verdict":"partial"}]}"#);
    let mut features = vec![feature("f")];
    let mut doubtful = step(2, Some("a.rs"));
    doubtful.action = "calls missing_fn".to_string();
    let mut use_cases = vec![use_case("f", vec![passing_step(), doubtful])];

    score_confidence(
        dir.path(),
        &mut features,
        &mut use_cases,
        &provider,
        None,
        1,
    )
    .await
    .unwrap();

    let prompt = &provider.prompts()[0];
    assert!(prompt.contains("2. ") && prompt.contains("missing_fn"));
    assert!(!prompt.contains("1. "));
    // (1.0 + 0.5) / 2
    assert_eq!(use_cases[0].confidence.as_ref().unwrap().value, 0.75);
}

#[tokio::test]
async fn a_sample_gives_a_second_opinion_on_that_many_passing_use_cases() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::answering(r#"{"steps":[{"order":1,"verdict":"partial"}]}"#);
    let mut features = vec![feature("f")];
    let mut use_cases: Vec<UseCase> = (0..6)
        .map(|_| use_case("f", vec![passing_step()]))
        .collect();

    score_confidence(
        dir.path(),
        &mut features,
        &mut use_cases,
        &provider,
        Some(2),
        1,
    )
    .await
    .unwrap();

    // Every use case is scored; the two sampled ones got the model's opinion.
    let values: Vec<f32> = use_cases
        .iter()
        .map(|u| u.confidence.as_ref().unwrap().value)
        .collect();
    assert_eq!(values.iter().filter(|v| **v == 0.5).count(), 2);
    assert_eq!(values.iter().filter(|v| **v == 1.0).count(), 4);
    let again = FakeLlm::answering("unused");
    score_confidence(
        dir.path(),
        &mut features,
        &mut use_cases,
        &again,
        Some(2),
        1,
    )
    .await
    .unwrap();
    assert_eq!(again.calls(), 0, "a rerun is a no-op");
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

    score_confidence(
        dir.path(),
        &mut features,
        &mut use_cases,
        &provider,
        None,
        1,
    )
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

    score_confidence(dir.path(), &mut features, &mut use_cases, &spy, None, 1)
        .await
        .unwrap();

    let prompts = spy.prompts();
    assert!(prompts[0].contains("def action_50"), "cited code is shown");
    assert!(prompts[0].contains("omitted)"));
    assert!(!prompts[0].contains("def action_30"));
}

// spec: RER-3
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
        score_confidence(
            dir.path(),
            &mut features,
            &mut use_cases,
            &provider,
            None,
            1,
        )
        .await
        .unwrap();
        calls.push(provider.calls());
    }
    assert_eq!(calls, [1, 0, 1]);
    assert!(use_cases[0].confidence.is_some());
}

/// A verdict per call: every step supported, except for the feature named `weak` (partial).
fn verdicts_by_feature(weak: &'static str) -> FakeLlm {
    FakeLlm::replying(move |_, request| {
        let verdict = if request.messages[1].content.contains(weak) {
            "partial"
        } else {
            "supported"
        };
        Ok(format!(
            r#"{{"steps":[{{"order":1,"verdict":"{verdict}"}}]}}"#
        ))
    })
    .with_delay(std::time::Duration::from_millis(30))
}

fn four_features(dir: &Path) -> (Vec<Feature>, Vec<UseCase>) {
    std::fs::write(dir.join("a.rs"), "fn a() {}\n").unwrap();
    let names = ["one", "two", "three", "four"];
    let features = names.iter().map(|n| feature(n)).collect();
    let use_cases = names
        .iter()
        .map(|n| {
            let mut u = use_case(n, vec![step(1, Some("a.rs"))]);
            u.name = format!("Use case of {n}");
            u
        })
        .collect();
    (features, use_cases)
}

#[tokio::test]
async fn use_cases_scored_in_parallel_get_the_scores_of_a_sequential_run() {
    let mut runs = Vec::new();
    for concurrency in [1, 3] {
        let dir = tempfile::tempdir().unwrap();
        let (mut features, mut use_cases) = four_features(dir.path());
        let provider = verdicts_by_feature("three");

        score_confidence(
            dir.path(),
            &mut features,
            &mut use_cases,
            &provider,
            None,
            concurrency,
        )
        .await
        .unwrap();

        let scores: Vec<f32> = use_cases
            .iter()
            .map(|u| u.confidence.as_ref().unwrap().value)
            .collect();
        runs.push((scores, provider.most_in_flight()));
    }

    assert_eq!(runs[0].0, [1.0, 1.0, 0.5, 1.0]);
    assert_eq!(runs[0].0, runs[1].0);
    assert_eq!(runs[0].1, 1);
    assert!((2..=3).contains(&runs[1].1), "in flight: {}", runs[1].1);
}

#[tokio::test]
async fn a_failure_among_parallel_scores_leaves_the_earlier_use_cases_scored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut features, mut use_cases) = four_features(dir.path());
    let flaky = FakeLlm::replying(|n, _| {
        if n == 2 {
            Err(retrodoc_llm::LlmError::Transport("down".to_string()))
        } else {
            Ok(r#"{"steps":[{"order":1,"verdict":"supported"}]}"#.to_string())
        }
    })
    .with_delay(std::time::Duration::from_millis(30));

    let result = score_confidence(dir.path(), &mut features, &mut use_cases, &flaky, None, 3).await;

    assert!(result.is_err());
    let scored: Vec<bool> = use_cases.iter().map(|u| u.confidence.is_some()).collect();
    assert_eq!(scored, [true, true, false, false]);
}

#[test]
fn mechanical_score_is_known_only_when_no_step_is_doubtful() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();

    let settled = use_case("f", vec![passing_step()]);
    let (tally, score) = mechanical_score(dir.path(), &settled, &[]);
    assert_eq!((tally.passing, tally.doubtful), (1, 0));
    assert_eq!(score.unwrap().value, 1.0);

    let open = use_case("f", vec![passing_step(), step(2, Some("a.rs"))]);
    let (tally, score) = mechanical_score(dir.path(), &open, &[]);
    assert_eq!((tally.passing, tally.doubtful), (1, 1));
    assert!(score.is_none());
}

// spec: RER-5
#[tokio::test]
async fn a_killed_run_keeps_the_scores_given_and_the_rerun_scores_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let (mut features, mut use_cases) = four_features(dir.path());
    let supported = |_: usize| r#"{"steps":[{"order":1,"verdict":"supported"}]}"#.to_string();

    let stuck = FakeLlm::hanging_after(2, supported);
    killed(
        score_confidence(dir.path(), &mut features, &mut use_cases, &stuck, None, 1),
        &stuck,
    )
    .await;
    let mut saved = load_use_cases(dir.path()).expect("the use cases scored are on disk");
    let scored: Vec<bool> = saved.iter().map(|u| u.confidence.is_some()).collect();
    assert_eq!(scored, [true, true, false, false]);

    let healthy = FakeLlm::replying(move |n, _| Ok(supported(n)));
    score_confidence(dir.path(), &mut features, &mut saved, &healthy, None, 1)
        .await
        .unwrap();
    assert_eq!(healthy.calls(), 2, "only the two use cases left unscored");
    assert!(saved.iter().all(|u| u.confidence.is_some()));
}
