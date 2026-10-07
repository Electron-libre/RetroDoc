use super::*;

use crate::testing::FakeLlm;

fn feature(slug: &str, paths: &[&str]) -> Feature {
    Feature {
        slug: slug.to_string(),
        domain_slug: "billing".to_string(),
        sub_domain_slug: None,
        name: slug.to_string(),
        description: "d".to_string(),
        source_paths: paths.iter().map(ToString::to_string).collect(),
        confidence: None,
    }
}

#[test]
fn cited_paths_resolve_to_a_unique_feature_file() {
    let allowed: BTreeSet<String> = ["m/src/schema.rs", "m/src/spec.rs", "x/src/spec.rs"]
        .map(String::from)
        .into();
    let resolve = |c| resolve_cited_path(c, &allowed);
    assert_eq!(
        resolve("m/src/schema.rs").as_deref(),
        Some("m/src/schema.rs")
    );
    assert_eq!(
        resolve("./src/schema.rs").as_deref(),
        Some("m/src/schema.rs")
    );
    assert_eq!(resolve("src/spec.rs"), None); // ambiguous
    assert_eq!(resolve("src/other.rs"), None);
}

#[test]
fn numbered_excerpt_numbers_lines_and_truncates() {
    let content = "a\nb\nc\n";
    assert_eq!(
        numbered_excerpt(content, 1000),
        "   1 | a\n   2 | b\n   3 | c\n"
    );
    assert!(numbered_excerpt(content, 1).contains("(truncated)"));
}

#[tokio::test]
async fn build_use_cases_renumbers_steps_and_drops_ungrounded_refs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::answering(
        r#"{"use_cases":[
          {"slug":"Pay invoice","name":"Pay invoice","description":"d","steps":[
            {"description":"s1","actor":{"name":"Customer","kind":"human"},
             "action":"submits payment",
             "source_refs":[{"path":"a.rs","start_line":1,"end_line":1},
                            {"path":"ghost.rs","start_line":null,"end_line":null}]},
            {"description":"s2","actor":{"name":"API","kind":"system"},
             "action":"records payment","source_refs":[]}]},
          {"slug":"empty","name":"Empty","description":"d","steps":[]}
        ]}"#,
    );

    let use_cases = build_use_cases(
        dir.path(),
        &[feature("payment", &["a.rs"])],
        &UseCaseContext::default(),
        &provider,
    )
    .await
    .unwrap();

    assert_eq!(use_cases.len(), 1);
    let uc = &use_cases[0];
    assert_eq!(uc.slug, "pay-invoice");
    assert_eq!(uc.feature_slug, "payment");
    assert_eq!(
        uc.steps.iter().map(|s| s.order).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(uc.steps[0].source_refs.len(), 1);
    assert_eq!(uc.steps[0].source_refs[0].path, "a.rs");
    assert!(is_human(&uc.steps[0].actor));
    assert!(!is_human(&uc.steps[1].actor));

    assert_eq!(load_use_cases(dir.path()).unwrap().len(), 1);
}

#[tokio::test]
async fn build_use_cases_skips_features_without_readable_files_and_bad_answers() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::answering("not json");

    let use_cases = build_use_cases(
        dir.path(),
        &[
            feature("missing", &["nope.rs"]),
            feature("garbled", &["a.rs"]),
        ],
        &UseCaseContext::default(),
        &provider,
    )
    .await
    .unwrap();

    assert!(use_cases.is_empty());
}

#[tokio::test]
async fn build_use_cases_tolerates_sloppy_steps_and_references() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    // Two fenced blocks (only the first counts), an empty step, a step
    // without actor, a note instead of a reference, a free-form actor kind.
    let provider = FakeLlm::answering(
        r#"```json
        {"use_cases":[{"slug":"u","name":"U","description":"d","steps":[
          {},
          {"description":"no actor","action":"x"},
          {"description":"ok","actor":{"name":"Dev","kind":"Human"},"action":"does",
           "source_refs":[{"note":"inferred"},{"path":"a.rs"}]}]}]}
        ```
        ```json
        {"use_cases":[]}
        ```"#,
    );

    let use_cases = build_use_cases(
        dir.path(),
        &[feature("f", &["a.rs"])],
        &UseCaseContext::default(),
        &provider,
    )
    .await
    .unwrap();

    assert_eq!(use_cases.len(), 1);
    let steps = &use_cases[0].steps;
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].order, 1);
    assert!(is_human(&steps[0].actor));
    assert_eq!(steps[0].source_refs.len(), 1);
    assert_eq!(steps[0].source_refs[0].path, "a.rs");
}

#[tokio::test]
async fn rerun_reuses_use_cases_until_a_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
    let features = vec![feature("pay", &["a.rs"])];
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"u","name":"U","description":"d","steps":[
            {"description":"s","actor":{"name":"A","kind":"human"},"action":"act",
             "source_refs":[{"path":"a.rs"}]}]}]}"#,
    );
    let calls = || provider.calls();

    let mut first = build_use_cases(dir.path(), &features, &UseCaseContext::default(), &provider)
        .await
        .unwrap();
    assert_eq!(calls(), 1);
    // Scored use cases keep their score when reused.
    first[0].confidence = Some(retrodoc_core::model::ConfidenceScore::new(0.9, None));
    save_use_cases(dir.path(), &first).unwrap();

    let again = build_use_cases(dir.path(), &features, &UseCaseContext::default(), &provider)
        .await
        .unwrap();
    assert_eq!(calls(), 1, "unchanged feature must not call the LLM");
    assert_eq!(again.len(), 1);
    assert!(again[0].confidence.is_some());

    std::fs::write(dir.path().join("a.rs"), "fn a() { changed }").unwrap();
    let redone = build_use_cases(dir.path(), &features, &UseCaseContext::default(), &provider)
        .await
        .unwrap();
    assert_eq!(calls(), 2, "a changed file must invalidate the feature");
    assert!(redone[0].confidence.is_none());
}

#[tokio::test]
async fn a_clean_empty_answer_is_remembered_until_the_feature_changes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
    let features = vec![feature("idle", &["a.rs"])];
    let provider = FakeLlm::answering(r#"{"use_cases":[]}"#);
    let context = UseCaseContext::default();
    let run = || build_use_cases(dir.path(), &features, &context, &provider);

    assert!(run().await.unwrap().is_empty());
    assert_eq!(provider.calls(), 2, "an empty answer is asked once more");

    assert!(run().await.unwrap().is_empty());
    assert_eq!(
        provider.calls(),
        2,
        "unchanged feature must not call the LLM"
    );

    std::fs::write(dir.path().join("a.rs"), "fn a() { changed }").unwrap();
    run().await.unwrap();
    assert_eq!(
        provider.calls(),
        4,
        "a changed file must invalidate the feature"
    );
}

#[tokio::test]
async fn lost_saved_use_cases_are_asked_again_despite_the_fingerprints() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
    let features = vec![feature("idle", &["a.rs"])];
    let provider = FakeLlm::answering(r#"{"use_cases":[]}"#);
    let context = UseCaseContext::default();

    build_use_cases(dir.path(), &features, &context, &provider)
        .await
        .unwrap();
    std::fs::remove_file(Artifact::UseCases.path(dir.path())).unwrap();
    build_use_cases(dir.path(), &features, &context, &provider)
        .await
        .unwrap();
    assert_eq!(provider.calls(), 4, "no saved use cases: nothing to trust");
}

#[tokio::test]
async fn use_cases_all_dropped_by_grounding_are_remembered() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
    let features = vec![feature("idle", &["a.rs"])];
    // The only use case has no step, so grounding drops it.
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"u","name":"U","description":"d","steps":[]}]}"#,
    );
    let context = UseCaseContext::default();
    let run = || build_use_cases(dir.path(), &features, &context, &provider);

    assert!(run().await.unwrap().is_empty());
    assert_eq!(provider.calls(), 1);
    assert!(run().await.unwrap().is_empty());
    assert_eq!(
        provider.calls(),
        1,
        "same input, same result: not asked again"
    );
}

#[tokio::test]
async fn an_empty_answer_followed_by_a_garbled_one_is_retried_on_rerun() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
    let features = vec![feature("flaky", &["a.rs"])];
    let provider = FakeLlm::answering("not json");
    let context = UseCaseContext::default();
    let run = || build_use_cases(dir.path(), &features, &context, &provider);

    run().await.unwrap();
    let first = provider.calls();
    run().await.unwrap();
    assert!(
        provider.calls() > first,
        "an unparseable answer is not remembered"
    );
}

#[tokio::test]
async fn a_feature_with_entry_points_gets_them_and_the_code_they_run() {
    use crate::entry_points::{EntryFile, EntryKind, Output, OutputKind};

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (path, content) in [
        (
            "app/contracts_controller.rb",
            "def sign\n  ContractSigner.call\nend\n",
        ),
        ("app/contract_signer.rb", "class ContractSigner\nend\n"),
        ("app/unrelated.rb", "class Unrelated\nend\n"),
    ] {
        std::fs::create_dir_all(root.join("app")).unwrap();
        std::fs::write(root.join(path), content).unwrap();
    }
    let entry_points = EntryPoints {
        files: BTreeMap::from([(
            PathBuf::from("app/contracts_controller.rb"),
            EntryFile {
                content_hash: String::new(),
                entry_points: vec![EntryPoint {
                    kind: EntryKind::HttpRoute,
                    name: "POST /contracts/:id/sign".to_string(),
                    verb: "sign".to_string(),
                    resource: "contract".to_string(),
                    description: "A signatory signs".to_string(),
                    outputs: vec![Output {
                        kind: OutputKind::Email,
                        description: "confirmation sent".to_string(),
                    }],
                }],
            },
        )]),
    };
    let index = CodeIndex::new(
        [
            "app/contracts_controller.rb",
            "app/contract_signer.rb",
            "app/unrelated.rb",
        ]
        .iter()
        .map(Path::new),
    );
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"sign","name":"Sign a contract","description":"d",
          "entry_points":["post /contracts/:id/sign","GET /ghost"],
          "steps":[{"description":"s","actor":{"name":"Signatory","kind":"human"},
            "action":"signs","source_refs":[{"path":"app/contract_signer.rb"}]}]}]}"#,
    );
    // The feature only lists the controller: the signer is outside it.
    let features = [feature("signing", &["app/contracts_controller.rb"])];

    let context = UseCaseContext {
        entry_points,
        index,
        ..UseCaseContext::default()
    };
    let use_cases = build_use_cases(root, &features, &context, &provider)
        .await
        .unwrap();

    let prompts = provider.prompt_pairs();
    assert!(prompts[0].0.contains("entry_points"));
    assert!(prompts[0]
        .1
        .contains("- POST /contracts/:id/sign (app/contracts_controller.rb): A signatory signs"));
    assert!(prompts[0].1.contains("Email confirmation sent"));
    assert!(prompts[0].1.contains("=== app/contract_signer.rb ==="));
    assert!(!prompts[0].1.contains("unrelated.rb"));
    // Known entry points are kept as the inventory spells them, unknown dropped;
    // a step may cite code reached through the entry point.
    assert_eq!(use_cases[0].entry_points, vec!["POST /contracts/:id/sign"]);
    assert_eq!(
        use_cases[0].steps[0].source_refs[0].path,
        "app/contract_signer.rb"
    );
}

#[tokio::test]
async fn known_actors_reach_the_prompt_and_name_the_steps() {
    use crate::actors::BusinessActor;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let actors = Actors {
        input_hash: String::new(),
        actors: vec![
            BusinessActor {
                name: "Signatory".to_string(),
                kind: ActorKind::Human,
                description: "Signs contracts".to_string(),
                evidence: Vec::new(),
            },
            BusinessActor {
                name: "E-signature provider".to_string(),
                kind: ActorKind::System,
                description: "Collects signatures".to_string(),
                evidence: Vec::new(),
            },
        ],
    };
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"sign","name":"Sign","description":"d","primary_actor":"SIGNATORY","narrative":"  A signatory signs the contract.  ","steps":[
          {"description":"s1","actor":{"name":"signatory","kind":"system"},"action":"signs"},
          {"description":"s2","actor":{"name":"e-signature PROVIDER","kind":"human"},"action":"records"},
          {"description":"s3","actor":{"name":"Developer","kind":"human"},"action":"reads"}]}]}"#,
    );

    let use_cases = build_use_cases(
        dir.path(),
        &[feature("signing", &["a.rs"])],
        &UseCaseContext {
            actors,
            ..UseCaseContext::default()
        },
        &provider,
    )
    .await
    .unwrap();

    let prompts = provider.prompt_pairs();
    assert!(prompts[0].0.contains("known actors"));
    assert!(prompts[0]
        .1
        .contains("- Signatory (human): Signs contracts"));
    let steps = &use_cases[0].steps;
    // Known actors take the list's spelling *and* kind; others are kept as given.
    assert_eq!(
        (steps[0].actor.name.as_str(), steps[0].actor.kind),
        ("Signatory", ActorKind::Human)
    );
    assert_eq!(
        (steps[1].actor.name.as_str(), steps[1].actor.kind),
        ("E-signature provider", ActorKind::System)
    );
    assert_eq!(steps[2].actor.name, "Developer");
    // The primary actor must be a known one, spelled as in the list.
    assert_eq!(use_cases[0].primary_actor.as_deref(), Some("Signatory"));
    // The narrative is stored trimmed; the prompt asks for it and for the vocabulary.
    assert_eq!(
        use_cases[0].narrative.as_deref(),
        Some("A signatory signs the contract.")
    );
    assert!(prompts[0].0.contains("`narrative`"));
}

/// A text field the model returns as an object (typically `primary_actor` copied from the
/// shape of a step's `actor`) costs that field, not the feature, and without a retry.
#[tokio::test]
async fn text_fields_given_as_objects_do_not_reject_the_answer() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"sign","name":"Sign",
          "description":{"text":"Signs a contract."},
          "primary_actor":{"name":"Signatory","kind":"human"},
          "narrative":{},
          "steps":[{"description":"s1","actor":{"name":"Signatory","kind":"human"},
                    "action":{"name":"signs","detail":"on the page"}}]}]}"#,
    );

    let use_cases = build_use_cases(
        dir.path(),
        &[feature("signing", &["a.rs"])],
        &UseCaseContext::default(),
        &provider,
    )
    .await
    .unwrap();

    assert_eq!(provider.calls(), 1, "no retry");
    assert_eq!(use_cases.len(), 1);
    assert_eq!(use_cases[0].description, "Signs a contract.");
    assert_eq!(use_cases[0].narrative, None, "an empty object is no text");
    assert_eq!(use_cases[0].steps.len(), 1);
}

#[test]
fn lenient_text_accepts_strings_and_objects() {
    use super::grounding::RawUseCase;
    let parse = |json: &str| -> RawUseCase { serde_json::from_str(json).unwrap() };
    let base = r#""slug":"s","name":"n""#;

    let string = parse(&format!(r#"{{{base},"primary_actor":"Signatory"}}"#));
    assert_eq!(string.primary_actor.as_deref(), Some("Signatory"));

    let named = parse(&format!(
        r#"{{{base},"primary_actor":{{"name":"Signatory","kind":"human"}}}}"#
    ));
    assert_eq!(named.primary_actor.as_deref(), Some("Signatory"));

    let unnamed = parse(&format!(
        r#"{{{base},"primary_actor":{{"role":"Admin","level":3}}}}"#
    ));
    assert_eq!(
        unnamed.primary_actor.as_deref(),
        Some("Admin"),
        "string values joined, others dropped"
    );

    let kind_only = parse(&format!(r#"{{{base},"primary_actor":{{"kind":"human"}}}}"#));
    assert_eq!(kind_only.primary_actor, None, "a kind is not a name");

    let unusable = parse(&format!(
        r#"{{{base},"primary_actor":{{"level":3}},"narrative":null}}"#
    ));
    assert_eq!(unusable.primary_actor, None);
    assert_eq!(unusable.narrative, None);
}

/// The answer of the call of rank `n`: one use case named after it.
fn use_case_reply(n: usize) -> String {
    format!(
        r#"{{"use_cases":[{{"slug":"uc{n}","name":"U","description":"d","steps":[
        {{"description":"s","actor":{{"name":"A","kind":"human"}},"action":"x","source_refs":[]}}]}}]}}"#
    )
}

#[tokio::test]
async fn a_failed_run_keeps_the_features_done_and_the_rerun_resumes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(dir.path().join("b.rs"), "fn b() {}\n").unwrap();
    let features = [feature("one", &["a.rs"]), feature("two", &["b.rs"])];
    let context = UseCaseContext::default();

    let flaky = FakeLlm::failing_after(1, use_case_reply);
    assert!(build_use_cases(dir.path(), &features, &context, &flaky)
        .await
        .is_err());
    assert_eq!(load_use_cases(dir.path()).unwrap().len(), 1);

    // The first feature is cached: the second one is the call of rank 1.
    let healthy = FakeLlm::replying(|n, _| Ok(use_case_reply(n + 1)));
    let use_cases = build_use_cases(dir.path(), &features, &context, &healthy)
        .await
        .unwrap();
    assert_eq!(use_cases.len(), 2);
    // Only the second feature was sent to the LLM again.
    assert_eq!(healthy.calls(), 1);
}

#[tokio::test]
async fn a_long_controller_is_shown_around_the_actions_of_its_entry_points() {
    use crate::entry_points::{EntryFile, EntryKind};
    use std::fmt::Write as _;

    let dir = tempfile::tempdir().unwrap();
    let mut code = String::from("class ContractsController\n\n");
    for i in 1..=60 {
        let name = if i == 55 {
            "send_contract".to_string()
        } else {
            format!("action_{i}")
        };
        let _ = writeln!(code, "  def {name}");
        for step in 1..=8 {
            let _ = writeln!(code, "    work_{i}_{step}");
        }
        let _ = writeln!(code, "  end\n");
    }
    std::fs::write(dir.path().join("contracts_controller.rb"), &code).unwrap();
    let entry_points = EntryPoints {
        files: BTreeMap::from([(
            PathBuf::from("contracts_controller.rb"),
            EntryFile {
                content_hash: String::new(),
                entry_points: vec![EntryPoint {
                    kind: EntryKind::HttpRoute,
                    name: "POST /contracts/:id/send_contract".to_string(),
                    verb: "send".to_string(),
                    resource: "contract".to_string(),
                    description: "Sends the contract".to_string(),
                    outputs: Vec::new(),
                }],
            },
        )]),
    };
    let context = UseCaseContext {
        entry_points,
        index: CodeIndex::new([Path::new("contracts_controller.rb")]),
        ..UseCaseContext::default()
    };
    let provider = FakeLlm::answering(r#"{"use_cases":[]}"#);
    let features = [feature("sending", &["contracts_controller.rb"])];

    build_use_cases(dir.path(), &features, &context, &provider)
        .await
        .unwrap();

    let prompts = provider.prompt_pairs();
    let prompt = &prompts[0].1;
    assert!(prompt.contains("def send_contract"), "the action is shown");
    assert!(
        prompt.contains("class ContractsController"),
        "so is the header"
    );
    assert!(prompt.contains("omitted)"));
    assert!(!prompt.contains("def action_20"));
}

#[tokio::test]
async fn a_first_empty_answer_recovered_by_the_retry_gives_its_use_case() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let features = vec![feature("late", &["a.rs"])];
    let provider = FakeLlm::sequence(&[
        r#"{"use_cases":[]}"#,
        r#"{"use_cases":[{"slug":"do-it","name":"Do it","description":"d","steps":[
            {"description":"s1","actor":{"name":"Customer","kind":"human"},
             "action":"does it","source_refs":[]}]}]}"#,
    ]);

    let use_cases = build_use_cases(dir.path(), &features, &UseCaseContext::default(), &provider)
        .await
        .unwrap();

    assert_eq!(provider.calls(), 2);
    assert_eq!(use_cases.len(), 1);
    assert_eq!(use_cases[0].slug, "do-it");
}
