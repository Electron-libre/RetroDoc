use retrodoc_core::model::{ConfidenceScore, Domain, Feature};
use retrodoc_llm::ToolCall;
use retrodoc_pipeline::testing::{FakeLlm, FakeTurn};
use retrodoc_pipeline::Glossary;

use super::*;

fn docs() -> Arc<Docs> {
    let domain = Domain {
        slug: "billing".into(),
        name: "Billing".into(),
        description: "Invoices and payments".into(),
        confidence: Some(ConfidenceScore::new(0.8, None)),
    };
    let feature = Feature {
        back_office: false,
        technical: false,
        slug: "pay-invoice".into(),
        domain_slug: "billing".into(),
        name: "Pay an invoice".into(),
        description: "Settle what is due".into(),
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

fn call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        arguments: arguments.into(),
    }
}

fn calls(calls: Vec<ToolCall>) -> FakeTurn {
    FakeTurn::Calls {
        text: String::new(),
        calls,
    }
}

const READ_TOOLS: [&str; 7] = [
    "list_domains",
    "get_domain",
    "get_feature",
    "get_use_case",
    "search_docs",
    "read_source",
    "git_log",
];

// spec: ASK-1, ASK-2 (internal: the requests of the model are seen through the fake provider only)
#[tokio::test]
async fn the_model_is_offered_the_seven_read_tools_and_none_that_writes() {
    let llm = FakeLlm::scripted(vec![FakeTurn::Text("Billing.".into())]);
    ask(&llm, docs(), "What is billed?", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();
    assert_eq!(llm.tools(), vec![READ_TOOLS.map(String::from).to_vec()]);
    let specs = tool_specs();
    assert!(specs
        .iter()
        .all(|t| !t.description.is_empty() && t.parameters["type"] == "object"));
}

// spec: ASK-3, ASK-5 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn each_call_gets_the_answer_of_the_tool_and_the_text_answer_ends_the_question() {
    let llm = FakeLlm::scripted(vec![
        calls(vec![
            call("c1", "list_domains", "{}"),
            call("c2", "get_feature", r#"{"id":"billing/pay-invoice"}"#),
        ]),
        FakeTurn::Text("Invoices are paid in `billing/pay-invoice`.".into()),
    ]);
    let docs = docs();
    let answer = ask(&llm, Arc::clone(&docs), "How is an invoice paid?", 8)
        .await
        .unwrap();

    assert_eq!(answer.text, "Invoices are paid in `billing/pay-invoice`.");
    assert_eq!(answer.rounds, 1);
    let expected = vec![docs.list_domains(), docs.get_feature("billing/pay-invoice")];
    assert_eq!(answer.tool_results, expected);
    let second = &llm.messages()[1];
    let results: Vec<(&str, &str)> = second
        .iter()
        .filter(|m| m.role == Role::Tool)
        .map(|m| (m.tool_call_id.as_deref().unwrap(), m.content.as_str()))
        .collect();
    assert_eq!(
        results,
        [("c1", expected[0].as_str()), ("c2", expected[1].as_str())]
    );
    assert_eq!(llm.calls(), 2);
}

// spec: ASK-4 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn a_bad_call_is_answered_with_its_error_and_the_question_goes_on() {
    let llm = FakeLlm::scripted(vec![
        calls(vec![
            call("c1", "get_feature", r#"{"slug":"x"}"#),
            call("c2", "edit_section", "{}"),
            call("c3", "get_domain", "not json"),
        ]),
        FakeTurn::Text("Not documented.".into()),
    ]);
    let answer = ask(&llm, docs(), "?", 8).await.unwrap();

    assert_eq!(answer.text, "Not documented.");
    let [missing, unknown, invalid] = answer.tool_results.as_slice() else {
        panic!("three results: {:?}", answer.tool_results);
    };
    assert!(
        missing.starts_with("Error:") && missing.contains("`id`"),
        "{missing}"
    );
    assert!(
        unknown.starts_with("Error: unknown tool `edit_section`"),
        "{unknown}"
    );
    assert!(
        invalid.starts_with("Error:") && invalid.contains("not JSON"),
        "{invalid}"
    );
}

// spec: ASK-6 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn after_the_last_round_the_model_is_asked_to_answer_without_tools() {
    let llm = FakeLlm::replying(|n, request| {
        Ok(if request.tools.is_empty() {
            format!("Answer at call {n}.")
        } else {
            String::new()
        })
    });
    // A model that calls a tool at every round it is offered them.
    let looping = FakeLlm::scripted(vec![calls(vec![call("c", "list_domains", "{}")])]);
    let answer = ask(&looping, docs(), "?", 2).await.unwrap();

    assert_eq!(answer.rounds, 2);
    let offered: Vec<usize> = looping.tools().iter().map(Vec::len).collect();
    assert_eq!(offered, [7, 7, 0]);
    let last = looping.messages().pop().unwrap();
    assert_eq!(last.last().unwrap().role, Role::User);
    assert!(last
        .last()
        .unwrap()
        .content
        .contains("Answer the question now"));
    // Without tools, the text of that last answer is the answer.
    let answer = ask(&llm, docs(), "?", 0).await.unwrap();
    assert_eq!(answer.text, "Answer at call 0.");
    assert_eq!(answer.rounds, 0);
}
