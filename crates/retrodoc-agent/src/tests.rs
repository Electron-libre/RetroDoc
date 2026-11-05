use retrodoc_core::model::{ConfidenceScore, Domain, Feature};
use retrodoc_llm::ToolCall;
use retrodoc_pipeline::testing::{FakeLlm, FakeTurn};
use retrodoc_pipeline::Glossary;

use retrodoc_mcp::SourceKind;

use super::*;

fn docs() -> Arc<Docs> {
    docs_with(Vec::new())
}

/// The docs of one domain `billing` and its feature `pay-invoice`, plus `more` features.
fn docs_with(more: Vec<Feature>) -> Arc<Docs> {
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
    let mut features = vec![feature];
    features.extend(more);
    Arc::new(Docs::new(
        vec![domain],
        features,
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

fn source(kind: SourceKind, id: &str) -> Source {
    Source {
        kind,
        id: id.into(),
    }
}

// spec: ASK-7, ASK-8 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn the_sources_are_the_names_of_the_text_a_tool_result_held() {
    let llm = FakeLlm::scripted(vec![
        calls(vec![call("c1", "get_feature", r#"{"id":"pay-invoice"}"#)]),
        FakeTurn::Text(
            "Paid in `billing/pay-invoice` (`app/invoice.rb:3`, `app/invoice.rb`); refunds in \
             `billing/refund`; see `billing` and `pay-invoice`."
                .into(),
        ),
    ]);
    let answer = ask(&llm, docs(), "How is an invoice paid?", 8)
        .await
        .unwrap();

    assert_eq!(
        answer.sources,
        [
            source(SourceKind::Feature, "billing/pay-invoice"),
            source(SourceKind::File, "app/invoice.rb"),
            source(SourceKind::Domain, "billing"),
        ]
    );
}

// spec: ASK-8, ASK-9 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn an_answer_whose_names_no_tool_showed_has_no_source_and_says_so() {
    let llm = FakeLlm::scripted(vec![FakeTurn::Text("It is `billing/pay-invoice`.".into())]);
    let answer = ask(&llm, docs(), "?", 8).await.unwrap();

    assert!(answer.sources.is_empty(), "{:?}", answer.sources);
    let shown = answer.render();
    assert!(
        shown.starts_with("It is `billing/pay-invoice`.\n"),
        "{shown}"
    );
    assert!(shown.contains(NO_SOURCE), "{shown}");
}

// spec: ASK-7 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn the_rendered_answer_lists_its_sources_after_its_text() {
    let llm = FakeLlm::scripted(vec![
        calls(vec![call("c1", "list_domains", "{}")]),
        FakeTurn::Text("See `billing`.".into()),
    ]);
    let shown = ask(&llm, docs(), "?", 8).await.unwrap().render();

    assert_eq!(shown, "See `billing`.\n\nSources:\n- domain `billing`\n");
}

// spec: ASK-17 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn a_name_only_a_failed_call_or_a_search_echo_held_is_no_source() {
    let llm = FakeLlm::scripted(vec![
        calls(vec![
            call("c1", "get_feature", r#"{"id":"shop/pay-invoice"}"#),
            call("c2", "search_docs", r#"{"query":"refunds"}"#),
            call("c3", "get_domain", r#"{"id":"nowhere"}"#),
        ]),
        FakeTurn::Text("See `pay-invoice`, `billing/pay-invoice` and `billing`.".into()),
    ]);
    let answer = ask(&llm, docs(), "?", 8).await.unwrap();

    assert!(
        answer.tool_results[0].starts_with("Not documented"),
        "{:?}",
        answer.tool_results
    );
    assert!(answer.tool_results[1].starts_with("Not documented"));
    assert!(answer.tool_results[2].contains("Known domains: billing"));
    assert_eq!(answer.sources, []);
}

// spec: ASK-7 (internal: the code spans are read from a text the test gives)
#[test]
fn a_span_closes_only_on_a_run_of_as_many_backticks() {
    let text = "Run ``a ` b`` then:\n```ruby\n`ls`\n```\nand `` ` `` or `billing` and `app/x.rb`.";
    let spans: Vec<&str> = code_spans(text).collect();
    assert_eq!(spans, ["a ` b", "`", "billing", "app/x.rb"]);
}

// spec: ASK-18 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn a_blank_answer_says_the_model_gave_none() {
    let llm = FakeLlm::scripted(vec![calls(vec![call("c", "list_domains", "{}")])]);
    let shown = ask(&llm, docs(), "?", 1).await.unwrap().render();

    assert!(shown.starts_with(NO_ANSWER), "{shown}");
}

// spec: ASK-20 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn null_arguments_are_an_empty_object() {
    let llm = FakeLlm::scripted(vec![
        calls(vec![call("c", "list_domains", "null")]),
        FakeTurn::Text("Billing.".into()),
    ]);
    let docs = docs();
    let answer = ask(&llm, Arc::clone(&docs), "?", 8).await.unwrap();

    assert_eq!(answer.tool_results, [docs.list_domains()]);
}

// spec: ASK-21 (internal: the tools offered are seen through the fake provider only; the option is the bench's)
#[tokio::test]
async fn without_the_code_tools_neither_is_offered_named_nor_run() {
    let llm = FakeLlm::scripted(vec![
        calls(vec![
            call("c1", "read_source", r#"{"path":"app/invoice.rb"}"#),
            call("c2", "search", r#"{"query":"invoice"}"#),
        ]),
        FakeTurn::Text("Not documented.".into()),
    ]);
    let answer = ask_with(&llm, docs(), "How is an invoice paid?", 8, Tools::DocsOnly)
        .await
        .unwrap();
    let docs_tools: Vec<String> = READ_TOOLS[..5].iter().map(ToString::to_string).collect();
    assert_eq!(llm.tools()[0], docs_tools);
    let system = &llm.system_prompts()[0];
    assert!(
        !system.contains("read_source") && !system.contains("git_log"),
        "{system}"
    );
    let offered = "the tools are list_domains, get_domain, get_feature, get_use_case, search_docs.";
    for (result, tool) in answer.tool_results.iter().zip(["read_source", "search"]) {
        assert_eq!(*result, format!("Error: unknown tool `{tool}`; {offered}"));
    }
}

// spec: TCL-19 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn the_reasoning_of_a_round_that_called_tools_is_given_back_in_the_next() {
    let llm = FakeLlm::scripted(vec![
        FakeTurn::Reasoned {
            reasoning: "look up the domains".into(),
            calls: vec![call("c1", "list_domains", "{}")],
        },
        FakeTurn::Text("Billing.".into()),
    ]);
    ask(&llm, docs(), "What is billed?", DEFAULT_MAX_ROUNDS)
        .await
        .unwrap();

    let second = &llm.messages()[1];
    let assistant = second.iter().find(|m| m.role == Role::Assistant).unwrap();
    assert_eq!(assistant.reasoning.as_deref(), Some("look up the domains"));
}

// spec: ASK-22 (internal: the messages of the last request are seen through the fake provider only)
#[tokio::test]
async fn the_last_request_holds_the_calls_and_results_as_text_only() {
    let llm = FakeLlm::scripted(vec![calls(vec![call("c1", "list_domains", "{}")])]);
    let docs = docs();
    ask(&llm, Arc::clone(&docs), "What is billed?", 1)
        .await
        .unwrap();

    let last = llm.messages().pop().unwrap();
    let roles: Vec<Role> = last.iter().map(|m| m.role).collect();
    assert_eq!(roles, [Role::System, Role::User, Role::User]);
    assert!(last.iter().all(|m| m.tool_calls.is_empty()));
    assert_eq!(last[1].content, "What is billed?");
    let folded = &last[2].content;
    for part in [
        "`list_domains`",
        "`{}`",
        docs.list_domains().as_str(),
        "Answer the question now",
    ] {
        assert!(folded.contains(part), "{part} not in {folded}");
    }
    assert!(folded.find("`list_domains`") < folded.find("Answer the question now"));
}

// spec: ASK-23 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn the_calls_of_a_round_past_the_sixth_are_refused() {
    let seven = (1..=7)
        .map(|n| call(&format!("c{n}"), "list_domains", "{}"))
        .collect();
    let llm = FakeLlm::scripted(vec![calls(seven), FakeTurn::Text("Billing.".into())]);
    let docs = docs();
    let answer = ask(&llm, Arc::clone(&docs), "?", 8).await.unwrap();

    let (run, refused) = answer.tool_results.split_at(6);
    assert!(run.iter().all(|r| *r == docs.list_domains()), "{run:?}");
    let [refused] = refused else {
        panic!("seven results: {:?}", answer.tool_results);
    };
    assert!(
        refused.starts_with("Error:") && refused.contains("6 calls") && refused.contains("answer"),
        "{refused}"
    );
}

/// The docs of [`docs`] with two more features whose descriptions are `first` and `second`
/// characters long.
fn docs_with_long_features(first: usize, second: usize) -> Arc<Docs> {
    let feature = |slug: &str, length: usize| Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: "billing".into(),
        name: slug.into(),
        description: "x".repeat(length),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: None,
    };
    docs_with(vec![feature("huge", first), feature("large", second)])
}

// spec: ASK-24 (internal: the rounds are scripted with the fake provider, which the binary cannot take)
#[tokio::test]
async fn a_result_past_the_budget_of_the_question_is_refused_with_its_size() {
    let docs = docs_with_long_features(58_900, 2000);
    let llm = FakeLlm::scripted(vec![
        calls(vec![
            call("c1", "get_feature", r#"{"id":"billing/huge"}"#),
            call("c2", "get_feature", r#"{"slug":"x"}"#),
            call("c3", "get_feature", r#"{"id":"billing/large"}"#),
            call("c4", "list_domains", "{}"),
        ]),
        FakeTurn::Text("Billing.".into()),
    ]);
    let answer = ask(&llm, Arc::clone(&docs), "?", 8).await.unwrap();

    let used = docs.get_feature("billing/huge").chars().count();
    let large = docs.get_feature("billing/large").chars().count();
    assert!(used + large > 60_000 && used < 60_000);
    let [huge, _error, refused, small] = answer.tool_results.as_slice() else {
        panic!("four results: {:?}", answer.tool_results);
    };
    assert_eq!(*huge, docs.get_feature("billing/huge"));
    assert!(
        refused.starts_with("Error:")
            && refused.contains(&large.to_string())
            && refused.contains(&(60_000 - used).to_string())
            && refused.contains("answer"),
        "{}",
        &refused[..refused.len().min(200)]
    );
    assert_eq!(*small, docs.list_domains());
}
