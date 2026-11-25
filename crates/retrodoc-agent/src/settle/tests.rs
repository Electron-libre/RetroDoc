use std::sync::Arc;

use retrodoc_core::artifacts::Glossary;
use retrodoc_core::model::{Domain, Feature};
use retrodoc_docs::reviews::{Section, SectionKind, Status};
use retrodoc_docs::Docs;
use retrodoc_llm::{Role, ToolCall};
use retrodoc_pipeline::testing::{FakeLlm, FakeTurn};

use super::{
    proposal_of, Disagreement, Disputed, Proposal, PROPOSE_VERDICT, SETTLING_LAST_ROUND,
    SETTLING_PROMPT,
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
                    r#"{"status":"wrong","comment":"c","narrative":"n"}"#,
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
        r#"{"status":"to_review","comment":" c ","narrative":"n","name":" "}"#,
    )
    .unwrap();
    assert_eq!(
        accepted,
        Proposal {
            section: use_case,
            status: Status::ToReview,
            comment: "c".into(),
            name: None,
            description: None,
            narrative: Some("n".into()),
        }
    );
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
                r#"{"status":"wrong","comment":"`app/invoice.rb` charges the card.","description":"The customer pays online."}"#,
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
    assert_eq!(proposal.status, Status::Wrong);
    assert_eq!(
        proposal.description.as_deref(),
        Some("The customer pays online.")
    );
    assert_eq!(answer.text, "`app/invoice.rb` charges the card.");

    let said = FakeLlm::scripted(vec![calls(
        "The section is right.",
        vec![call(
            "c1",
            PROPOSE_VERDICT,
            r#"{"status":"validated","comment":"c"}"#,
        )],
    )]);
    let answer = Conversation::settling(feature())
        .ask(&said, docs(), "Settle it.", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();
    assert_eq!(answer.text, "The section is right.");
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
                r#"{"status":"wrong","comment":"The customer pays.","name":"Pay online"}"#,
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
        .contains("Proposed verdict: wrong\nComment: The customer pays.\nName: Pay online"));
}

// spec: STL-5 (internal: the requests of the model are seen through the fake provider only)
#[tokio::test]
async fn the_last_round_of_a_settling_asks_the_verdict_as_text() {
    let llm = FakeLlm::scripted(vec![
        calls(
            "",
            vec![call("c1", "get_feature", r#"{"slug":"pay-invoice"}"#)],
        ),
        FakeTurn::Text("Wrong: the customer pays.".into()),
    ]);
    let answer = Conversation::settling(feature())
        .ask(&llm, docs(), "Help me settle it.", 1)
        .await
        .unwrap();
    assert_eq!(answer.proposal, None);
    let last = llm.messages().pop().unwrap();
    let asked = &last.last().unwrap().content;
    assert!(asked.ends_with(SETTLING_LAST_ROUND), "{asked}");
    assert_eq!(llm.tools().last().map(Vec::len), Some(0));

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
    let last = plain.messages().pop().unwrap();
    assert!(!last.last().unwrap().content.contains(SETTLING_LAST_ROUND));
}
