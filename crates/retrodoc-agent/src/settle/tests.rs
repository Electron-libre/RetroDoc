use std::sync::Arc;

use retrodoc_core::artifacts::Glossary;
use retrodoc_core::model::{Domain, Feature};
use retrodoc_docs::reviews::{Section, SectionKind, Status};
use retrodoc_docs::Docs;
use retrodoc_llm::{Role, ToolCall};
use retrodoc_pipeline::testing::{FakeLlm, FakeTurn};

use super::{
    proposal_of, propose_verdict_spec, Disagreement, Disputed, Proposal, Refusal,
    MAX_COMMENT_CHARS, PROPOSE_VERDICT, SETTLING_LAST_ROUND, SETTLING_PROMPT,
};
use crate::{Conversation, DEFAULT_MAX_ROUNDS};

/// The docs of one domain `billing` and its feature `pay-invoice`.
fn docs() -> Arc<Docs> {
    let domain = Domain {
        slug: "billing".into(),
        name: "Billing".into(),
        description: "Invoices and payments".into(),
        confidence: None,
    };
    let feature = Feature {
        back_office: false,
        technical: false,
        slug: "pay-invoice".into(),
        domain_slug: "billing".into(),
        name: "Pay an invoice".into(),
        description: "The clerk settles what is due".into(),
        source_paths: vec!["app/invoice.rb".into()],
        entry_points: Vec::new(),
        confidence: None,
    };
    Arc::new(Docs::new(
        vec![domain],
        vec![feature],
        Vec::new(),
        &Glossary::default(),
        &[],
    ))
}

fn feature() -> Section {
    Section::new(SectionKind::Feature, "pay-invoice")
}

fn call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        arguments: arguments.into(),
    }
}

fn calls(text: &str, calls: Vec<ToolCall>) -> FakeTurn {
    FakeTurn::Calls {
        text: text.into(),
        calls,
    }
}

// spec: STL-4 (internal: the message is the first question the web sends, its text seen here whole)
#[test]
fn the_settling_message_holds_the_section_as_its_tool_answers_and_its_contradictions() {
    let docs = docs();
    let disputed = Disputed {
        section: feature(),
        disagreements: vec![Disagreement {
            doc_section: "docs/billing.md#Payment".into(),
            doc_states: "The customer pays online.".into(),
            section_states: "The clerk settles what is due.".into(),
        }],
    };
    let message = disputed.message(&docs);
    assert!(message.contains("feature `pay-invoice`"), "{message}");
    assert!(
        message.contains(docs.get_feature("pay-invoice").trim_end()),
        "{message}"
    );
    assert!(message.contains("`docs/billing.md#Payment` states: The customer pays online."));
    assert!(message.contains("The section states: The clerk settles what is due."));
    let alone = Disputed {
        section: Section::new(SectionKind::Domain, "billing"),
        disagreements: Vec::new(),
    }
    .message(&docs);
    assert!(alone.contains("domain `billing`"), "{alone}");
    assert!(alone.contains(docs.get_domain("billing").trim_end()));
    assert!(alone.ends_with("The project's docs contradict none of it."));
}

// spec: STL-5, STL-6 (internal: the requests of the model are seen through the fake provider only)
#[tokio::test]
async fn a_settling_sends_the_verification_prompt_and_offers_propose_verdict_each_question() {
    let llm = FakeLlm::scripted(vec![
        FakeTurn::Text("Who pays, the clerk or the customer?".into()),
        FakeTurn::Text("Thanks.".into()),
    ]);
    let mut settling = Conversation::settling(feature());
    assert_eq!(settling.settled(), Some(&feature()));
    for question in ["Help me settle it.", "The customer."] {
        settling
            .ask(&llm, docs(), question, DEFAULT_MAX_ROUNDS)
            .await
            .unwrap();
    }
    for prompt in llm.system_prompts() {
        assert!(prompt.contains(SETTLING_PROMPT), "{prompt}");
        assert!(
            prompt.ends_with("the feature `pay-invoice`: read it again with get_feature when the conversation no longer shows it."),
            "{prompt}"
        );
        assert!(prompt.starts_with("You answer questions"), "{prompt}");
    }
    for tools in llm.tools() {
        assert_eq!(tools.len(), 8, "{tools:?}");
        assert_eq!(tools.last().map(String::as_str), Some(PROPOSE_VERDICT));
    }

    let plain = FakeLlm::scripted(vec![FakeTurn::Text("Billing.".into())]);
    let mut conversation = Conversation::default();
    assert_eq!(conversation.settled(), None);
    conversation
        .ask(&plain, docs(), "?", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();
    assert!(!plain.tools()[0].iter().any(|t| t == PROPOSE_VERDICT));
    assert!(!plain.system_prompts()[0].contains(SETTLING_PROMPT));
}

// spec: STL-13 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn a_proposal_the_table_refuses_gets_its_error_and_the_question_goes_on() {
    let llm = FakeLlm::scripted(vec![
        calls(
            "",
            vec![
                call(
                    "c1",
                    PROPOSE_VERDICT,
                    r#"{"status":"false","comment":"no"}"#,
                ),
                call("c2", PROPOSE_VERDICT, r#"{"status":"wrong"}"#),
                call(
                    "c3",
                    PROPOSE_VERDICT,
                    r#"{"status":"validated","comment":"c","evidence":"e","narrative":"n"}"#,
                ),
                call("c4", PROPOSE_VERDICT, "not json"),
            ],
        ),
        FakeTurn::Text("I could not settle it.".into()),
    ]);
    let answer = Conversation::settling(feature())
        .ask(&llm, docs(), "Settle it.", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();
    assert_eq!(answer.text, "I could not settle it.");
    assert_eq!(answer.proposal, None);
    let [status, comment, narrative, json] = answer.tool_results.as_slice() else {
        panic!("four results: {:?}", answer.tool_results);
    };
    assert!(
        status.starts_with("Error:") && status.contains("`status`"),
        "{status}"
    );
    assert!(
        comment.starts_with("Error:") && comment.contains("`comment`"),
        "{comment}"
    );
    assert!(
        narrative.starts_with("Error:") && narrative.contains("no narrative"),
        "{narrative}"
    );
    assert!(json.starts_with("Error:"), "{json}");
    let use_case = Section::new(SectionKind::UseCase, "pay-invoice/pay-by-card");
    let accepted = proposal_of(
        &use_case,
        r#"{"status":"to_review","comment":" c ","evidence":" e ","narrative":"n","name":" "}"#,
    )
    .unwrap();
    assert_eq!(
        accepted,
        Proposal {
            section: use_case,
            status: Status::ToReview,
            comment: "c".into(),
            evidence: "e".into(),
            name: None,
            description: None,
            narrative: Some("n".into()),
        }
    );
}

// spec: STL-13 (internal: the arguments of a call are what the model sends, checked before any question)
#[test]
fn a_proposal_needs_its_evidence_a_short_comment_and_a_status_that_fits_its_corrections() {
    let section = feature();
    let refused = |arguments: &str| proposal_of(&section, arguments).unwrap_err();
    let no_evidence = refused(r#"{"status":"wrong","comment":"c"}"#);
    assert!(
        no_evidence.starts_with("Error:") && no_evidence.contains("`evidence`"),
        "{no_evidence}"
    );
    let comment = |length: usize| "é".repeat(length);
    let long = refused(&format!(
        r#"{{"status":"wrong","comment":"{}","evidence":"e"}}"#,
        comment(601)
    ));
    assert!(
        long.contains("600 characters") && long.contains("`evidence`"),
        "{long}"
    );
    assert!(
        long.contains("601 characters"),
        "the error names its length: {long}"
    );
    let short = proposal_of(
        &section,
        &format!(
            r#"{{"status":"wrong","comment":"{}","evidence":"e"}}"#,
            comment(600)
        ),
    );
    assert!(short.is_ok(), "{short:?}");
    let partial = proposal_of(
        &section,
        r#"{"status":"wrong","comment":"c","evidence":"e","description":"The customer pays."}"#,
    )
    .unwrap();
    assert_eq!(partial.status, Status::Wrong);
    assert_eq!(partial.description.as_deref(), Some("The customer pays."));
    let corrected = proposal_of(
        &section,
        r#"{"status":"validated","comment":"c","evidence":"e","description":"The customer pays."}"#,
    )
    .unwrap();
    assert_eq!(corrected.status, Status::Validated);
    assert_eq!(corrected.description.as_deref(), Some("The customer pays."));
}

// spec: STL-5, STL-13 (internal: the prompt and the tool are what the model reads, seen as sent)
#[test]
fn the_prompt_and_the_tool_say_how_to_write_the_comment_and_which_status_fits() {
    assert!(
        SETTLING_PROMPT.contains("not a developer"),
        "{SETTLING_PROMPT}"
    );
    assert!(SETTLING_PROMPT.contains("evidence"), "{SETTLING_PROMPT}");
    let tool = propose_verdict_spec(&feature());
    let status = tool.parameters["properties"]["status"]["description"]
        .as_str()
        .unwrap();
    for fits in ["validated", "wrong", "to_review"] {
        assert!(status.contains(fits), "{status}");
    }
    for meaning in ["once corrected", "you cannot correct"] {
        assert!(
            SETTLING_PROMPT.contains(meaning),
            "{meaning} in {SETTLING_PROMPT}"
        );
    }
    assert!(
        status.contains("corrected fields given") && status.contains("left uncorrected"),
        "a correction is validated, wrong leaves a claim uncorrected: {status}"
    );
    assert_eq!(
        tool.parameters["required"],
        serde_json::json!(["status", "comment", "evidence"])
    );
}

// spec: STL-13 (internal: the schema of the tool is what the model reads, seen as sent)
#[test]
fn the_schema_limits_the_comment_and_offers_a_narrative_to_a_use_case_only() {
    for kind in [
        SectionKind::Domain,
        SectionKind::Feature,
        SectionKind::UseCase,
    ] {
        let tool = propose_verdict_spec(&Section::new(kind, "pay-invoice"));
        let properties = &tool.parameters["properties"];
        assert_eq!(
            properties["comment"]["maxLength"],
            serde_json::json!(MAX_COMMENT_CHARS),
            "{kind:?}"
        );
        assert_eq!(
            properties.get("narrative").is_some(),
            kind == SectionKind::UseCase,
            "{kind:?}: {properties}"
        );
    }
}

// spec: STL-14 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn a_valid_proposal_ends_the_question_and_the_calls_after_it_are_not_run() {
    let llm = FakeLlm::scripted(vec![calls(
        "",
        vec![
            call(
                "c1",
                PROPOSE_VERDICT,
                r#"{"status":"validated","comment":"The customer pays.","evidence":"`app/invoice.rb` charges the card.","description":"The customer pays online."}"#,
            ),
            call("c2", "read_source", r#"{"path":"app/invoice.rb"}"#),
        ],
    )]);
    let steps = std::sync::Mutex::new(Vec::new());
    let answer = Conversation::settling(feature())
        .ask_observed(&llm, docs(), "Settle it.", DEFAULT_MAX_ROUNDS, &|step| {
            steps.lock().unwrap().push(step.tool);
        })
        .await
        .unwrap();
    assert_eq!(llm.calls(), 1);
    assert_eq!(*steps.lock().unwrap(), [PROPOSE_VERDICT]);
    let proposal = answer.proposal.unwrap();
    assert_eq!(proposal.status, Status::Validated);
    assert_eq!(
        proposal.description.as_deref(),
        Some("The customer pays online.")
    );
    assert_eq!(answer.text, "The customer pays.");

    let said = FakeLlm::scripted(vec![calls(
        "The section is right.",
        vec![call(
            "c1",
            PROPOSE_VERDICT,
            r#"{"status":"validated","comment":"c","evidence":"e"}"#,
        )],
    )]);
    let answer = Conversation::settling(feature())
        .ask(&said, docs(), "Settle it.", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();
    assert_eq!(answer.text, "The section is right.");
}

// spec: STL-14 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn the_sources_of_a_proposal_are_checked_over_its_evidence_too() {
    let llm = FakeLlm::scripted(vec![
        calls(
            "",
            vec![call("c1", "get_feature", r#"{"id":"pay-invoice"}"#)],
        ),
        calls(
            "",
            vec![call(
                "c2",
                PROPOSE_VERDICT,
                r#"{"status":"validated","comment":"The customer pays.","evidence":"`billing/pay-invoice` names the clerk.","description":"The customer pays online."}"#,
            )],
        ),
    ]);
    let answer = Conversation::settling(feature())
        .ask(&llm, docs(), "Settle it.", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();
    assert_eq!(answer.text, "The customer pays.");
    assert!(
        answer
            .sources
            .iter()
            .any(|source| source.id == "billing/pay-invoice"),
        "{:?} {:?}",
        answer.sources,
        answer.tool_results
    );
}

// spec: STL-15 (internal: the requests of the model are seen through the fake provider only)
#[tokio::test]
async fn the_kept_answer_of_a_proposal_holds_it_as_text() {
    let llm = FakeLlm::scripted(vec![
        calls(
            "Wrong, see the code.",
            vec![call(
                "c1",
                PROPOSE_VERDICT,
                r#"{"status":"validated","comment":"The customer pays.","evidence":"`app/invoice.rb`.","name":"Pay online"}"#,
            )],
        ),
        FakeTurn::Text("Kept to review.".into()),
    ]);
    let mut settling = Conversation::settling(feature());
    settling
        .ask(&llm, docs(), "Settle it.", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();
    settling
        .ask(&llm, docs(), "Keep it to review.", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();
    let second = &llm.messages()[1];
    let earlier = second
        .iter()
        .find(|m| m.role == Role::Assistant)
        .expect("the earlier answer");
    assert!(
        earlier.content.starts_with("Wrong, see the code."),
        "{}",
        earlier.content
    );
    assert!(earlier
        .content
        .contains("Proposed verdict: validated\nComment: The customer pays.\nEvidence: `app/invoice.rb`.\nName: Pay online"));
}

/// A settling of `pay-invoice` with one round, its tool calls then `last`, the answer of its last
/// request.
async fn after_its_one_round(last: FakeTurn) -> (FakeLlm, crate::Answer, Vec<String>) {
    after_its_round(
        calls(
            "",
            vec![call("c1", "get_feature", r#"{"slug":"pay-invoice"}"#)],
        ),
        last,
    )
    .await
}

/// A settling of `pay-invoice` with the one round `round` then `last`, the answer of its last
/// request, and the tools the observer was told.
async fn after_its_round(round: FakeTurn, last: FakeTurn) -> (FakeLlm, crate::Answer, Vec<String>) {
    let llm = FakeLlm::scripted(vec![round, last]);
    let told = std::sync::Mutex::new(Vec::new());
    let observer = |step: crate::Step| told.lock().unwrap().push(step.tool);
    let answer = Conversation::settling(feature())
        .ask_observed(&llm, docs(), "Help me settle it.", 1, &observer)
        .await
        .unwrap();
    (llm, answer, told.into_inner().unwrap())
}

// spec: STL-18 (internal: the requests of the model are seen through the fake provider only)
#[tokio::test]
async fn the_last_request_of_a_settling_offers_propose_verdict_alone() {
    let (llm, answer, told) = after_its_one_round(calls(
        "Settled.",
        vec![call(
            "p1",
            PROPOSE_VERDICT,
            r#"{"status":"validated","comment":"The customer pays.","evidence":"`app/invoice.rb`","description":"The customer pays."}"#,
        )],
    ))
    .await;
    assert_eq!(llm.tools().last(), Some(&vec![PROPOSE_VERDICT.to_string()]));
    let last = llm.messages().pop().unwrap();
    assert_eq!(last.len(), 2, "the system prompt and one user message");
    assert!(
        last[1].content.ends_with(SETTLING_LAST_ROUND),
        "{}",
        last[1].content
    );
    assert!(
        !last[1].content.contains(crate::LAST_ROUND),
        "{}",
        last[1].content
    );
    assert_eq!(told, ["get_feature", PROPOSE_VERDICT]);
    let proposal = answer.proposal.expect("the proposal of the last request");
    assert_eq!(proposal.status, Status::Validated);
    assert_eq!(answer.text, "Settled.");
    assert_eq!(answer.rounds, 1);

    let plain = FakeLlm::scripted(vec![
        calls(
            "",
            vec![call("c1", "get_feature", r#"{"slug":"pay-invoice"}"#)],
        ),
        FakeTurn::Text("Billing.".into()),
    ]);
    Conversation::default()
        .ask(&plain, docs(), "?", 1)
        .await
        .unwrap();
    assert_eq!(plain.tools().last().map(Vec::len), Some(0));
    let last = plain.messages().pop().unwrap();
    assert!(!last.last().unwrap().content.contains(SETTLING_LAST_ROUND));
}

// spec: STL-18 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn a_refused_proposal_or_another_tool_in_the_last_request_ends_on_its_text() {
    let long = "x".repeat(MAX_COMMENT_CHARS + 1);
    let refused = format!(r#"{{"status":"wrong","comment":"{long}","evidence":"e"}}"#);
    let not_json = proposal_of(&feature(), r#"{"status":"wr"#).unwrap_err();
    for (last, expected) in [
        (
            calls(
                "Wrong: the customer pays.",
                vec![call("p1", PROPOSE_VERDICT, &refused)],
            ),
            "Wrong: the customer pays.",
        ),
        (
            calls(
                "Who pays?",
                vec![call("r1", "read_source", r#"{"path":"app/invoice.rb"}"#)],
            ),
            "Who pays?",
        ),
        (FakeTurn::Text("Who pays?".into()), "Who pays?"),
        (
            calls("", vec![call("p1", PROPOSE_VERDICT, &refused)]),
            &long,
        ),
        (
            calls("", vec![call("p1", PROPOSE_VERDICT, r#"{"status":"wr"#)]),
            &not_json,
        ),
    ] {
        let (llm, answer, _) = after_its_one_round(last).await;
        assert_eq!(answer.proposal, None);
        assert_eq!(answer.text, expected);
        assert_eq!(llm.calls(), 2, "no round after the last request");
    }
}

// spec: STL-18 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn the_first_accepted_proposal_of_the_last_request_ends_the_question() {
    let long = "x".repeat(MAX_COMMENT_CHARS + 1);
    let refused = format!(r#"{{"status":"wrong","comment":"{long}","evidence":"e"}}"#);
    let accepted = r#"{"status":"validated","comment":"The customer pays.","evidence":"e","description":"The customer pays."}"#;
    let (_, answer, _) = after_its_one_round(calls(
        "",
        vec![
            call("p1", PROPOSE_VERDICT, &refused),
            call("p2", PROPOSE_VERDICT, accepted),
        ],
    ))
    .await;
    let proposal = answer.proposal.expect("the second call's proposal");
    assert_eq!(proposal.comment, "The customer pays.");
}

// spec: STL-18 (internal: the requests of the model are seen through the fake provider only)
#[tokio::test]
async fn the_last_request_holds_a_proposal_refused_in_a_round_and_its_error() {
    let long = "x".repeat(MAX_COMMENT_CHARS + 1);
    let refused = format!(r#"{{"status":"wrong","comment":"{long}","evidence":"e"}}"#);
    let (llm, _, _) = after_its_round(
        calls("", vec![call("p1", PROPOSE_VERDICT, &refused)]),
        FakeTurn::Text("Wrong.".into()),
    )
    .await;
    let last = llm.messages().pop().unwrap();
    let asked = &last[1].content;
    assert!(asked.contains("## Call of propose_verdict"), "{asked}");
    assert!(asked.contains(&long), "{asked}");
    assert!(
        asked.contains("Error: `comment` is longer than 600 characters"),
        "{asked}"
    );
    assert!(asked.ends_with(SETTLING_LAST_ROUND), "{asked}");
    assert!(!asked.contains(crate::LAST_ROUND), "{asked}");
}

// spec: STL-20 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn an_answer_keeps_each_refused_proposal_in_order_the_last_requests_included() {
    let long = "x".repeat(MAX_COMMENT_CHARS + 1);
    let too_long = format!(r#"{{"status":"wrong","comment":"{long}","evidence":"e"}}"#);
    let no_evidence = r#"{"status":"validated","comment":"Right."}"#;
    let accepted = r#"{"status":"validated","comment":"The customer pays.","evidence":"e"}"#;
    let (_, answer, _) = after_its_round(
        calls("", vec![call("p1", PROPOSE_VERDICT, &too_long)]),
        calls(
            "",
            vec![
                call("p2", PROPOSE_VERDICT, no_evidence),
                call("p3", PROPOSE_VERDICT, accepted),
            ],
        ),
    )
    .await;
    assert!(answer.proposal.is_some());
    assert_eq!(
        answer.refusals,
        [
            Refusal {
                status: Some("wrong".into()),
                comment_chars: MAX_COMMENT_CHARS + 1,
                error: proposal_of(&feature(), &too_long).unwrap_err(),
            },
            Refusal {
                status: Some("validated".into()),
                comment_chars: 6,
                error: proposal_of(&feature(), no_evidence).unwrap_err(),
            },
        ]
    );

    let (_, answer, _) =
        after_its_one_round(calls("", vec![call("p1", PROPOSE_VERDICT, accepted)])).await;
    assert_eq!(answer.refusals, []);

    let padded = format!(r#"{{"status":" wrong ","comment":" {long} ","evidence":"e"}}"#);
    let refusal = Refusal::of(&padded, String::new());
    assert_eq!(
        refusal.status.as_deref(),
        Some("wrong"),
        "trimmed as checked"
    );
    assert_eq!(
        refusal.comment_chars,
        MAX_COMMENT_CHARS + 1,
        "trimmed as checked"
    );
    let blank = Refusal::of(r#"{"status":"","comment":"   "}"#, String::new());
    assert_eq!((blank.status, blank.comment_chars), (None, 0));
}

// spec: STL-5 (internal: the system prompt is seen through the fake provider only)
#[tokio::test]
async fn the_settling_prompt_states_the_rounds_of_the_question() {
    for (rounds, said) in [
        (3, "You have 3 rounds of tool calls"),
        (1, "You have 1 round of tool calls"),
        (
            0,
            "You have no round of tool calls: only propose_verdict is offered",
        ),
    ] {
        let llm = FakeLlm::scripted(vec![FakeTurn::Text("Who pays?".into())]);
        Conversation::settling(feature())
            .ask(&llm, docs(), "Help me settle it.", rounds)
            .await
            .unwrap();
        let prompt = &llm.system_prompts()[0];
        assert!(prompt.contains(said), "{prompt}");
    }
}
