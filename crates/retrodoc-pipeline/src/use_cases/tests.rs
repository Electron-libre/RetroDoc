use super::*;

use crate::brief::Evidence;
use crate::testing::{killed, FakeLlm};
use crate::use_cases::grounding::{CALLING_PROGRAM, EXTERNAL_PROVIDER, SYSTEM};

fn feature(slug: &str, paths: &[&str]) -> Feature {
    Feature {
        back_office: false,
        slug: slug.to_string(),
        domain_slug: "billing".to_string(),
        sub_domain_slug: None,
        name: slug.to_string(),
        description: "d".to_string(),
        source_paths: paths.iter().map(ToString::to_string).collect(),
        entry_points: Vec::new(),
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
async fn steps_cite_the_files_by_their_number_in_the_prompt() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(dir.path().join("b.rs"), "fn b() {}\nfn c() {}\n").unwrap();
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"pay","name":"Pay","description":"d","steps":[
            {"description":"s","actor":{"name":"API","kind":"system"},"action":"pays",
             "code":["F2:1-2","F9:1","a.rs:1"]}]}]}"#,
    );

    let use_cases = build_use_cases(
        dir.path(),
        &[feature("payment", &["a.rs", "b.rs"])],
        &UseCaseContext::default(),
        &provider,
    )
    .await
    .unwrap();

    let prompts = provider.prompt_pairs();
    assert!(
        prompts[0].0.contains(r#""code":["F1:10-24"]"#),
        "{}",
        prompts[0].0
    );
    assert!(
        prompts[0].1.contains("=== [F1] a.rs ==="),
        "{}",
        prompts[0].1
    );
    assert!(prompts[0].1.contains("=== [F2] b.rs ==="));
    // The number of a file shown, else a path; an unknown number is dropped.
    let refs = &use_cases[0].steps[0].source_refs;
    assert_eq!(
        refs.iter()
            .map(|r| (r.path.as_str(), r.start_line, r.end_line))
            .collect::<Vec<_>>(),
        [("b.rs", Some(1), Some(2)), ("a.rs", Some(1), None)]
    );
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

// spec: RER-1, RER-2
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

// spec: RER-4
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

// spec: RER-9
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
    let (logs, _guard) = crate::testing::capture_logs();
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
    assert!(prompts[0].0.contains("the tags (`E1`"), "{}", prompts[0].0);
    assert!(prompts[0].1.contains(
        "- E1 POST /contracts/:id/sign [http_route] (app/contracts_controller.rb): A signatory signs"
    ));
    assert!(prompts[0].1.contains("Email confirmation sent"));
    assert!(prompts[0].1.contains("] app/contract_signer.rb ==="));
    assert!(!prompts[0].1.contains("unrelated.rb"));
    // Known entry points are kept as the inventory spells them, unknown dropped;
    // a step may cite code reached through the entry point.
    assert_eq!(
        use_cases[0].entry_points,
        vec!["app/contracts_controller.rb#POST /contracts/:id/sign"]
    );
    // A name that is no entry point costs nothing but the name: not a warning.
    let logs = logs.text();
    assert!(logs.contains("unknown entry point"), "{logs}");
    assert!(!logs.contains("WARN"), "{logs}");
    assert_eq!(
        use_cases[0].steps[0].source_refs[0].path,
        "app/contract_signer.rb"
    );
}

/// One use case of a feature with these entry point kinds (named `entry 0`, `entry 1`…), citing
/// `cited` of them (their numbers), with a human `Customer` step, a `Mailer` system step and
/// `primary` as primary actor; the actors list knows a human `Customer` and a software `Payment
/// provider`.
async fn use_case_on(
    kinds: &[crate::entry_points::EntryKind],
    cited: &[usize],
    primary: &str,
) -> UseCase {
    use crate::actors::BusinessActor;
    use crate::entry_points::EntryFile;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rb"), "def a\nend\n").unwrap();
    let entry_points = EntryPoints {
        files: BTreeMap::from([(
            PathBuf::from("a.rb"),
            EntryFile {
                content_hash: String::new(),
                entry_points: kinds
                    .iter()
                    .enumerate()
                    .map(|(i, kind)| EntryPoint {
                        kind: *kind,
                        name: format!("entry {i}"),
                        verb: String::new(),
                        resource: String::new(),
                        description: String::new(),
                        outputs: Vec::new(),
                    })
                    .collect(),
            },
        )]),
    };
    let cited: Vec<String> = cited.iter().map(|i| format!("entry {i}")).collect();
    let provider = FakeLlm::answering(format!(
        r#"{{"use_cases":[{{"slug":"go","name":"Go","description":"d","primary_actor":"{primary}",
          "entry_points":{},
          "steps":[{{"description":"s1","actor":{{"name":"Customer","kind":"human"}},
                    "action":"asks","source_refs":[]}},
                   {{"description":"s2","actor":{{"name":"Mailer","kind":"system"}},
                    "action":"sends","source_refs":[]}}]}}]}}"#,
        serde_json::to_string(&cited).unwrap()
    ));
    let known = |name: &str, kind| BusinessActor {
        name: name.to_string(),
        kind,
        description: String::new(),
        evidence: Vec::new(),
    };
    let context = UseCaseContext {
        entry_points,
        actors: Actors {
            input_hash: String::new(),
            actors: vec![
                known("Customer", ActorKind::Human),
                known("Payment provider", ActorKind::System),
            ],
        },
        ..UseCaseContext::default()
    };
    build_use_cases(dir.path(), &[feature("f", &["a.rb"])], &context, &provider)
        .await
        .unwrap()
        .remove(0)
}

#[tokio::test]
async fn no_human_actor_when_no_route_nor_command_starts_the_behaviour() {
    use crate::entry_points::EntryKind::{HttpRoute, Job, Other, PublicApi, Webhook};

    let api = use_case_on(&[PublicApi, PublicApi], &[], "Customer").await;
    assert_eq!(api.primary_actor, None, "a known human primary actor goes");
    assert_eq!(api.steps[0].actor.name, CALLING_PROGRAM);
    assert_eq!(api.steps[0].actor.kind, ActorKind::System);
    assert_eq!(api.steps[1].actor.name, "Mailer", "a system actor stays");

    let webhook = use_case_on(&[Webhook], &[], "Payment provider").await;
    assert_eq!(webhook.steps[0].actor.name, EXTERNAL_PROVIDER);
    assert_eq!(
        webhook.primary_actor.as_deref(),
        Some("Payment provider"),
        "a known software primary actor stays"
    );
    let job = use_case_on(&[Job, PublicApi], &[], "Customer").await;
    assert_eq!(job.steps[0].actor.name, SYSTEM);

    // A route or a command, or a kind nobody could name, leaves the human in place.
    for kinds in [&[HttpRoute, PublicApi][..], &[Other]] {
        let kept = use_case_on(kinds, &[], "Customer").await;
        assert_eq!(kept.primary_actor.as_deref(), Some("Customer"));
        assert_eq!(kept.steps[0].actor.kind, ActorKind::Human);
    }
    // No entry point at all: nothing says who triggers it.
    let unknown = use_case_on(&[], &[], "Customer").await;
    assert_eq!(unknown.steps[0].actor.kind, ActorKind::Human);
}

#[test]
fn the_prompt_says_who_starts_what_only_where_it_applies() {
    // The sentence about who starts a behaviour follows the entry points, the one about the
    // primary actor follows the known actors; a feature with neither gets none of them.
    let who_starts = "none of whose entry points is an http_route";
    let primary = "Give each use case a `primary_actor`";
    for (entries, actors) in [(true, true), (true, false), (false, true), (false, false)] {
        let prompt = prompt::system_prompt(entries, actors);
        assert_eq!(prompt.contains(who_starts), entries, "{entries} {actors}");
        assert_eq!(prompt.contains(primary), actors, "{entries} {actors}");
    }
    // `other` is a kind nobody could name: the check keeps its human, so does the prompt.
    assert!(prompt::system_prompt(true, false).contains("a cli_command or an other"));
    assert!(prompt::system_prompt(true, false).contains("also covers a route"));
    // The form and its submit are one use case, asked only where there are entry points.
    let one_use_case = "a route that only opens a form";
    assert!(prompt::system_prompt(true, false).contains(one_use_case));
    assert!(!prompt::system_prompt(false, true).contains(one_use_case));
}

#[tokio::test]
async fn the_entry_points_a_use_case_cites_decide_over_those_of_its_feature() {
    use crate::entry_points::EntryKind::{HttpRoute, Job};

    // `entry 1` is a job; `entry 0` a route that the use case does not cite.
    let job = use_case_on(&[HttpRoute, Job], &[1], "Customer").await;
    assert_eq!(job.steps[0].actor.name, SYSTEM);
    assert_eq!(job.primary_actor, None);
    let route = use_case_on(&[HttpRoute, Job], &[0], "Customer").await;
    assert_eq!(route.steps[0].actor.kind, ActorKind::Human);
    assert_eq!(route.primary_actor.as_deref(), Some("Customer"));
}

#[tokio::test]
async fn features_from_the_surface_get_only_their_own_entry_points_even_in_a_shared_file() {
    use crate::entry_points::{EntryFile, EntryKind};

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("api.rb"), "def a\nend\n").unwrap();
    let entry = |name: &str| EntryPoint {
        kind: EntryKind::HttpRoute,
        name: name.to_string(),
        verb: "get".to_string(),
        resource: "thing".to_string(),
        description: format!("does {name}"),
        outputs: Vec::new(),
    };
    let entry_points = EntryPoints {
        files: BTreeMap::from([(
            PathBuf::from("api.rb"),
            EntryFile {
                content_hash: String::new(),
                entry_points: vec![entry("GET /orders"), entry("GET /routes")],
            },
        )]),
    };
    let mut orders = feature("orders", &["api.rb"]);
    orders.entry_points = vec!["api.rb#GET /orders".to_string()];
    let mut routes = feature("routes", &["api.rb"]);
    routes.entry_points = vec!["api.rb#GET /routes".to_string()];
    let provider = FakeLlm::answering(r#"{"use_cases":[]}"#);
    let context = UseCaseContext {
        entry_points,
        index: CodeIndex::new(["api.rb"].iter().map(Path::new)),
        ..UseCaseContext::default()
    };

    build_use_cases(root, &[orders, routes], &context, &provider)
        .await
        .unwrap();

    let prompts = provider.prompt_pairs();
    assert!(prompts[0]
        .1
        .contains("- E1 GET /orders [http_route] (api.rb)"));
    assert!(!prompts[0].1.contains("GET /routes"));
    assert!(prompts[2]
        .1
        .contains("- E1 GET /routes [http_route] (api.rb)"));
    assert!(!prompts[2].1.contains("GET /orders"));
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

#[tokio::test]
async fn a_human_actor_outside_the_known_list_is_kept_and_only_an_info() {
    use crate::actors::BusinessActor;

    let (logs, _guard) = crate::testing::capture_logs();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let actors = Actors {
        input_hash: String::new(),
        actors: vec![BusinessActor {
            name: "Rider".to_string(),
            kind: ActorKind::Human,
            description: "Delivers".to_string(),
            evidence: Vec::new(),
        }],
    };
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"ride","name":"Ride","description":"d","steps":[
          {"description":"s1","actor":{"name":"riders","kind":"human"},"action":"rides"},
          {"description":"s2","actor":{"name":"Developer","kind":"human"},"action":"reads"}]}]}"#,
    );

    let use_cases = build_use_cases(
        dir.path(),
        &[feature("riding", &["a.rs"])],
        &UseCaseContext {
            actors,
            ..UseCaseContext::default()
        },
        &provider,
    )
    .await
    .unwrap();

    let steps = &use_cases[0].steps;
    // A plural or a separator is the same actor; a stranger is kept as given.
    assert_eq!(steps[0].actor.name, "Rider");
    assert_eq!(steps[1].actor.name, "Developer");
    let logs = logs.text();
    assert!(
        logs.contains("human actor outside the known actors"),
        "{logs}"
    );
    assert!(!logs.contains("WARN"), "{logs}");
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

// spec: RER-5
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

// spec: RER-5
#[tokio::test]
async fn a_killed_run_keeps_the_features_done_and_the_rerun_resumes() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.rs", "b.rs", "c.rs"] {
        std::fs::write(dir.path().join(name), format!("fn {name}() {{}}\n")).unwrap();
    }
    let features = [
        feature("one", &["a.rs"]),
        feature("two", &["b.rs"]),
        feature("three", &["c.rs"]),
    ];
    let context = UseCaseContext::default();

    let stuck = FakeLlm::hanging_after(1, use_case_reply);
    killed(
        build_use_cases(dir.path(), &features, &context, &stuck),
        &stuck,
    )
    .await;
    let saved = load_use_cases(dir.path()).expect("the use cases of the feature done are on disk");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].feature_slug, "one");

    let healthy = FakeLlm::replying(|n, _| Ok(use_case_reply(n + 1)));
    let use_cases = build_use_cases(dir.path(), &features, &context, &healthy)
        .await
        .unwrap();
    assert_eq!(use_cases.len(), 3);
    // Only the two features the killed run had not finished were sent to the LLM.
    assert_eq!(healthy.calls(), 2);
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

#[tokio::test]
async fn the_brief_heads_the_prompt_and_a_changed_brief_derives_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
    let features = vec![feature("idle", &["a.rs"])];
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"u","name":"U","description":"d","steps":[
            {"description":"s","actor":{"name":"A","kind":"human"},"action":"act",
             "source_refs":[{"path":"a.rs"}]}]}]}"#,
    );
    let with = |purpose: &str| UseCaseContext {
        brief: crate::testing::brief(purpose),
        ..UseCaseContext::default()
    };

    build_use_cases(dir.path(), &features, &with("Sells things."), &provider)
        .await
        .unwrap();
    let prompt = provider.prompt_pairs().remove(0).1;
    assert!(
        prompt.starts_with("Product brief of the application"),
        "{prompt}"
    );
    assert!(prompt.find("Purpose: Sells things.").unwrap() < prompt.find("Feature:").unwrap());

    build_use_cases(dir.path(), &features, &with("Sells things."), &provider)
        .await
        .unwrap();
    assert_eq!(provider.calls(), 1);

    build_use_cases(dir.path(), &features, &with("Rents things."), &provider)
        .await
        .unwrap();
    assert_eq!(provider.calls(), 2);
}

#[tokio::test]
async fn the_evidence_closest_to_a_feature_is_in_its_prompt_and_in_its_fingerprint() {
    use retrodoc_ingest::signals::{Signal, SignalKind};
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
    let mut late = feature("late", &["a.rs"]);
    late.name = "Late payment reminders".to_string();
    let features = vec![late];
    let provider = FakeLlm::answering(
        r#"{"use_cases":[{"slug":"u","name":"U","description":"d","steps":[
            {"description":"s","actor":{"name":"A","kind":"human"},"action":"act",
             "source_refs":[{"path":"a.rs"}]}]}]}"#,
    );
    let signal = |kind, origin: &str, text: &str| Signal {
        kind,
        origin: origin.to_string(),
        text: text.to_string(),
    };
    let reminders = signal(
        SignalKind::DocSection,
        "README.md#Reminders",
        "A reminder is sent when a payment is late.",
    );
    let with = |signals: &[Signal]| UseCaseContext {
        evidence: Evidence::new(signals),
        ..UseCaseContext::default()
    };

    build_use_cases(
        dir.path(),
        &features,
        &with(std::slice::from_ref(&reminders)),
        &provider,
    )
    .await
    .unwrap();
    let prompt = provider.prompt_pairs().remove(0).1;
    assert!(
        prompt.contains("A reminder is sent when a payment is late."),
        "{prompt}"
    );
    assert!(prompt.find("Project evidence").unwrap() < prompt.find("Source files:").unwrap());

    build_use_cases(
        dir.path(),
        &features,
        &with(std::slice::from_ref(&reminders)),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(provider.calls(), 1);

    let unrelated = signal(SignalKind::CommitSubject, "commit:1", "chore: bump ruby");
    build_use_cases(
        dir.path(),
        &features,
        &with(&[reminders.clone(), unrelated]),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(provider.calls(), 1, "an unrelated signal changes nothing");

    let related = signal(
        SignalKind::CommitSubject,
        "commit:2",
        "fix: late payment reminder",
    );
    build_use_cases(
        dir.path(),
        &features,
        &with(&[reminders, related]),
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(provider.calls(), 2);
}

// spec: RER-3
#[tokio::test]
async fn another_model_derives_the_unchanged_features_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
    let features = vec![feature("pay", &["a.rs"])];
    let answer = r#"{"use_cases":[{"slug":"u","name":"U","description":"d","steps":[
        {"description":"s","actor":{"name":"A","kind":"human"},"action":"act",
         "source_refs":[{"path":"a.rs"}]}]}]}"#;
    let run = |model: &'static str| {
        let provider = FakeLlm::answering(answer).with_model(model);
        let (root, features) = (dir.path().to_path_buf(), features.clone());
        async move {
            build_use_cases(&root, &features, &UseCaseContext::default(), &provider)
                .await
                .unwrap();
            provider.calls()
        }
    };
    assert_eq!(run("local").await, 1);
    assert_eq!(run("local").await, 0);
    assert_eq!(run("strong").await, 1);
}

/// A provider that answers one use case named after the feature of the prompt, after a short wait
/// (so that parallel calls overlap), and fails on the feature named `fail_on`.
fn probe(fail_on: Option<&'static str>) -> FakeLlm {
    FakeLlm::replying(move |_, request| {
        let feature = request.messages[1]
            .content
            .split("Feature: ")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .unwrap_or("?")
            .to_string();
        if fail_on == Some(feature.as_str()) {
            return Err(retrodoc_llm::LlmError::Transport("down".to_string()));
        }
        Ok(format!(
            r#"{{"use_cases":[{{"slug":"uc","name":"Use case of {feature}","description":"d","steps":[
            {{"description":"s","actor":{{"name":"A","kind":"human"}},"action":"x","source_refs":[]}}]}}]}}"#
        ))
    })
    .with_delay(std::time::Duration::from_millis(30))
}

fn features_with_files(dir: &Path, names: &[&str]) -> Vec<Feature> {
    names
        .iter()
        .map(|name| {
            std::fs::write(
                dir.join(format!("{name}.rs")),
                format!("fn {name}() {{}}\n"),
            )
            .unwrap();
            feature(name, &[&format!("{name}.rs")])
        })
        .collect()
}

#[tokio::test]
async fn use_cases_asked_in_parallel_come_out_in_the_order_of_the_features() {
    let names = ["one", "two", "three", "four", "five", "six"];
    let mut runs = Vec::new();
    for concurrency in [1, 3] {
        let dir = tempfile::tempdir().unwrap();
        let features = features_with_files(dir.path(), &names);
        let context = UseCaseContext {
            concurrency,
            ..UseCaseContext::default()
        };
        let probe = probe(None);
        let use_cases = build_use_cases(dir.path(), &features, &context, &probe)
            .await
            .unwrap();
        let summary: Vec<(String, String)> = use_cases
            .iter()
            .map(|u| (u.slug.clone(), u.name.clone()))
            .collect();
        runs.push((summary, probe.most_in_flight()));
    }

    // The same use cases in the order of the features.
    assert_eq!(runs[0].0, runs[1].0);
    let order: Vec<&str> = runs[1].0.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(
        order,
        [
            "Use case of one",
            "Use case of two",
            "Use case of three",
            "Use case of four",
            "Use case of five",
            "Use case of six"
        ]
    );
    assert_eq!(runs[0].1, 1, "sequential by default");
    assert!((2..=3).contains(&runs[1].1), "in flight: {}", runs[1].1);
}

#[tokio::test]
async fn a_failure_among_parallel_calls_keeps_the_features_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let features = features_with_files(dir.path(), &["one", "two", "three", "four"]);
    let context = UseCaseContext {
        concurrency: 3,
        ..UseCaseContext::default()
    };

    let flaky = probe(Some("three"));
    assert!(build_use_cases(dir.path(), &features, &context, &flaky)
        .await
        .is_err());
    let saved = load_use_cases(dir.path()).unwrap();
    assert_eq!(
        saved
            .iter()
            .map(|u| u.feature_slug.as_str())
            .collect::<Vec<_>>(),
        ["one", "two"]
    );

    // The rerun only asks for the two features that were not kept.
    let healthy = FakeLlm::replying(|n, _| Ok(use_case_reply(n + 10))).with_model("test-model");
    let use_cases = build_use_cases(dir.path(), &features, &context, &healthy)
        .await
        .unwrap();
    assert_eq!(use_cases.len(), 4);
    assert_eq!(healthy.calls(), 2);
}

#[tokio::test]
async fn the_vocabulary_of_another_domain_does_not_redo_the_use_cases_of_a_feature() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let features = vec![feature("pay", &["a.rs"])];
    let llm = FakeLlm::replying(|n, _| Ok(use_case_reply(n)));
    let with = |global: &[&str], billing: &[&str]| UseCaseContext {
        vocabulary: global.iter().map(ToString::to_string).collect(),
        domain_vocabulary: std::collections::BTreeMap::from([(
            "billing".to_string(),
            billing.iter().map(ToString::to_string).collect(),
        )]),
        ..UseCaseContext::default()
    };

    build_use_cases(dir.path(), &features, &with(&["Order"], &["Invoice"]), &llm)
        .await
        .unwrap();
    // Another context was read since: the application's vocabulary grew, the domain's did not.
    build_use_cases(
        dir.path(),
        &features,
        &with(&["Order", "Rider"], &["Invoice"]),
        &llm,
    )
    .await
    .unwrap();
    assert_eq!(llm.calls(), 1);

    build_use_cases(
        dir.path(),
        &features,
        &with(&["Order"], &["Invoice", "Ledger"]),
        &llm,
    )
    .await
    .unwrap();
    assert_eq!(llm.calls(), 2, "its own vocabulary does");
}

#[test]
fn an_entry_point_is_cited_by_tag_id_or_unambiguous_name() {
    use crate::entry_points::EntryKind::HttpRoute;

    let entry = |name: &str| EntryPoint {
        kind: HttpRoute,
        name: name.to_string(),
        verb: String::new(),
        resource: String::new(),
        description: String::new(),
        outputs: Vec::new(),
    };
    let entries = vec![
        (PathBuf::from("a.rb"), entry("GET /orders")),
        (PathBuf::from("b.rb"), entry("GET /orders")),
        (PathBuf::from("b.rb"), entry("POST /orders")),
    ];
    let cite = |names: &[&str]| {
        let cited: Vec<String> = names.iter().map(ToString::to_string).collect();
        grounding::known_entry_points(&cited, &entries)
    };
    assert_eq!(
        cite(&["E2", "e3"]),
        ["b.rb#GET /orders", "b.rb#POST /orders"]
    );
    assert_eq!(cite(&["a.rb#GET /orders"]), ["a.rb#GET /orders"]);
    assert_eq!(cite(&["post /orders"]), ["b.rb#POST /orders"]);
    // Twice the same, an ambiguous name, an unknown tag and an unknown name: kept once or dropped.
    assert_eq!(cite(&["E3", "b.rb#POST /orders"]), ["b.rb#POST /orders"]);
    assert_eq!(
        cite(&["GET /orders", "E9", "GET /ghost"]),
        Vec::<String>::new()
    );
}

/// A feature over `files`, each holding `per_file` routes named `GET /f<file>/<n>`; `answers` for
/// its calls. Returns the use cases and the provider.
async fn use_cases_of_many_entry_points(
    files: usize,
    per_file: usize,
    answers: &[&str],
) -> (Vec<UseCase>, FakeLlm) {
    use crate::entry_points::{EntryFile, EntryKind};

    let dir = tempfile::tempdir().unwrap();
    let names: Vec<String> = (0..files).map(|f| format!("f{f}.rb")).collect();
    for name in &names {
        std::fs::write(dir.path().join(name), "def a\nend\n").unwrap();
    }
    let entry_points = EntryPoints {
        files: names
            .iter()
            .enumerate()
            .map(|(f, name)| {
                let points = (0..per_file)
                    .map(|n| EntryPoint {
                        kind: EntryKind::HttpRoute,
                        name: format!("GET /f{f}/{n}"),
                        verb: String::new(),
                        resource: String::new(),
                        description: String::new(),
                        outputs: Vec::new(),
                    })
                    .collect();
                (
                    PathBuf::from(name),
                    EntryFile {
                        content_hash: String::new(),
                        entry_points: points,
                    },
                )
            })
            .collect(),
    };
    let provider = FakeLlm::sequence(answers);
    let context = UseCaseContext {
        entry_points,
        ..UseCaseContext::default()
    };
    let paths: Vec<&str> = names.iter().map(String::as_str).collect();
    let use_cases = build_use_cases(dir.path(), &[feature("big", &paths)], &context, &provider)
        .await
        .unwrap();
    (use_cases, provider)
}

fn one_use_case_citing(slug: &str, tags: &[&str]) -> String {
    format!(
        r#"{{"use_cases":[{{"slug":"{slug}","name":"{slug}","description":"d","entry_points":{},
          "steps":[{{"description":"s","actor":{{"name":"User","kind":"human"}},
                    "action":"a","source_refs":[]}}]}}]}}"#,
        serde_json::to_string(tags).unwrap()
    )
}

#[tokio::test]
async fn a_feature_with_many_entry_points_is_asked_in_slices_of_whole_files() {
    let answers = [
        one_use_case_citing("first", &["E1"]),
        one_use_case_citing("second", &["E1", "E10"]),
        one_use_case_citing("third", &["E10"]),
    ];
    let answers: Vec<&str> = answers.iter().map(String::as_str).collect();

    // Three files of 10 entry points: a file does not fit with the next one under the limit.
    let (use_cases, provider) = use_cases_of_many_entry_points(3, 10, &answers).await;

    let prompts = provider.prompt_pairs();
    assert_eq!(prompts.len(), 3);
    for (slice, (_, prompt)) in prompts.iter().enumerate() {
        assert!(
            prompt.contains(&format!("- E1 GET /f{slice}/0")),
            "{prompt}"
        );
        assert!(
            prompt.contains(&format!("- E10 GET /f{slice}/9")),
            "{prompt}"
        );
        assert!(!prompt.contains("- E11"), "{prompt}");
        assert_eq!(prompt.matches("GET /f").count(), 10, "{prompt}");
    }
    // The tags of each answer stand for the entry points of its own slice.
    let cited: Vec<(&str, Vec<&str>)> = use_cases
        .iter()
        .map(|u| (u.slug.as_str(), u.entry_point_names()))
        .collect();
    assert_eq!(
        cited,
        [
            ("first", vec!["GET /f0/0"]),
            ("second", vec!["GET /f1/0", "GET /f1/9"]),
            ("third", vec!["GET /f2/9"]),
        ]
    );
}

#[tokio::test]
async fn a_file_over_the_limit_is_cut_and_a_small_feature_is_asked_once() {
    let one = one_use_case_citing("u", &["E1"]);
    let (_, provider) = use_cases_of_many_entry_points(1, 20, &[&one, &one]).await;
    let prompts = provider.prompt_pairs();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].1.contains("- E12 GET /f0/11"));
    assert!(!prompts[0].1.contains("GET /f0/12"));
    assert!(prompts[1].1.contains("- E1 GET /f0/12"));
    assert!(prompts[1].1.contains("- E8 GET /f0/19"));

    let (_, provider) = use_cases_of_many_entry_points(2, 6, &[&one]).await;
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn a_slice_without_a_usable_answer_leaves_the_feature_to_the_next_run() {
    let ok = one_use_case_citing("u", &["E1"]);
    // Every call after the first answers with no JSON: the second slice is unusable.
    let (use_cases, provider) = use_cases_of_many_entry_points(3, 10, &[&ok, "no"]).await;

    assert!(use_cases.is_empty());
    assert!(provider.prompts().iter().all(|p| !p.contains("GET /f2/")));
}
