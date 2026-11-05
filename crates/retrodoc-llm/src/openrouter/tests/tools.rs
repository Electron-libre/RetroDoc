//! Tool calls on the wire (`specs/tool-calling.md`).

use super::{http, provider, request, serve, OK_BODY};
use crate::{ChatMessage, CompletionRequest, LlmProvider, Reasoning, Role, ToolCall, ToolSpec};

fn list_domains() -> ToolSpec {
    ToolSpec {
        name: "list_domains".to_string(),
        description: "List the domains of the generated docs".to_string(),
        parameters: serde_json::json!({"type": "object", "properties": {}}),
    }
}

/// Sends `request` to a server answering `payload`, returns the JSON body it received and the answer.
async fn exchange(
    request: CompletionRequest,
    payload: &str,
) -> (serde_json::Value, crate::CompletionResponse) {
    let (url, rx) = serve(vec![http("200 OK", payload)]);
    let response = provider(&url).complete(request).await.unwrap();
    let raw = rx.recv().unwrap();
    let body = &raw[raw.find("\r\n\r\n").unwrap() + 4..];
    (serde_json::from_str(body).unwrap(), response)
}

// spec: TCL-1 (internal: the wire body is only visible to the mock server of the provider's tests)
#[tokio::test]
async fn a_request_offering_tools_sends_each_as_a_function() {
    let (body, _) = exchange(
        CompletionRequest {
            tools: vec![list_domains()],
            ..request()
        },
        OK_BODY,
    )
    .await;

    assert_eq!(
        body["tools"],
        serde_json::json!([{
            "type": "function",
            "function": {
                "name": "list_domains",
                "description": "List the domains of the generated docs",
                "parameters": {"type": "object", "properties": {}}
            }
        }])
    );
}

// spec: TCL-2 (internal: the wire body is only visible to the mock server of the provider's tests)
#[tokio::test]
async fn a_request_offering_no_tool_sends_no_tools_field() {
    let (body, _) = exchange(request(), OK_BODY).await;
    assert!(body.get("tools").is_none(), "{body}");
}

// spec: TCL-3, TCL-4 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn the_tool_calls_of_an_answer_reach_the_caller_in_order_with_an_empty_text() {
    let (_, response) = exchange(
        CompletionRequest {
            tools: vec![list_domains()],
            ..request()
        },
        r#"{"model":"m","choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,
            "tool_calls":[
              {"id":"call_1","type":"function","function":{"name":"list_domains","arguments":"{}"}},
              {"id":"call_2","type":"function","function":{"name":"get_feature","arguments":"{\"id\": \"billing\"}"}}
            ]}}]}"#,
    )
    .await;

    assert_eq!(response.content, "");
    assert_eq!(
        response.tool_calls,
        vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "list_domains".to_string(),
                arguments: "{}".to_string(),
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "get_feature".to_string(),
                arguments: r#"{"id": "billing"}"#.to_string(),
            },
        ]
    );
}

// spec: TCL-4 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn an_answer_with_calls_and_no_content_field_is_an_empty_text() {
    let (_, response) = exchange(
        request(),
        r#"{"model":"m","choices":[{"message":{"role":"assistant",
            "tool_calls":[{"id":"c","type":"function","function":{"name":"t","arguments":"{}"}}]}}]}"#,
    )
    .await;
    assert_eq!(response.content, "");
    assert_eq!(response.tool_calls.len(), 1);
}

// spec: TCL-7 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn arguments_that_are_not_json_reach_the_caller_unchanged() {
    let (_, response) = exchange(
        request(),
        r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"",
            "tool_calls":[{"id":"c","type":"function","function":{"name":"t","arguments":"{\"id\": bill"}}]}}]}"#,
    )
    .await;
    assert_eq!(response.tool_calls[0].arguments, r#"{"id": bill"#);
}

// spec: TCL-5, TCL-6 (internal: the wire body is only visible to the mock server of the provider's tests)
#[tokio::test]
async fn the_calls_and_results_of_a_history_are_sent_back() {
    let call = ToolCall {
        id: "call_1".to_string(),
        name: "list_domains".to_string(),
        arguments: "{}".to_string(),
    };
    let (body, _) = exchange(
        CompletionRequest {
            messages: vec![
                ChatMessage::new(Role::User, "Which domains?"),
                ChatMessage::assistant_calls("", vec![call]),
                ChatMessage::tool_result("call_1", "- Billing"),
            ],
            tools: vec![list_domains()],
            ..request()
        },
        OK_BODY,
    )
    .await;

    assert_eq!(
        body["messages"],
        serde_json::json!([
            {"role": "user", "content": "Which domains?"},
            {"role": "assistant", "content": null, "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "list_domains", "arguments": "{}"}
            }]},
            {"role": "tool", "content": "- Billing", "tool_call_id": "call_1"}
        ])
    );
}

/// The answer of a server sending `message`, the message of its only choice.
async fn answer_to(message: &str) -> Result<crate::CompletionResponse, crate::LlmError> {
    let (url, _rx) = serve(vec![http(
        "200 OK",
        &format!(r#"{{"model":"m","choices":[{{"message":{message}}}]}}"#),
    )]);
    provider(&url).complete(request()).await
}

// spec: TCL-11 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn a_null_tool_calls_field_is_a_text_answer() {
    let response = answer_to(r#"{"role":"assistant","content":"ok","tool_calls":null}"#)
        .await
        .unwrap();
    assert_eq!(response.content, "ok");
    assert_eq!(response.tool_calls, []);
}

// spec: TCL-12 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn a_message_with_no_text_nor_call_is_an_invalid_response() {
    for message in [
        r#"{"role":"assistant","content":null}"#,
        r#"{"role":"assistant"}"#,
    ] {
        let result = answer_to(message).await;
        assert!(
            matches!(result, Err(crate::LlmError::InvalidResponse(_))),
            "{message}: {result:?}"
        );
    }
}

// spec: TCL-13 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn null_or_missing_arguments_are_an_empty_object() {
    let response = answer_to(
        r#"{"role":"assistant","tool_calls":[
            {"id":"a","type":"function","function":{"name":"t","arguments":null}},
            {"id":"b","type":"function","function":{"name":"t"}}]}"#,
    )
    .await
    .unwrap();
    let arguments: Vec<_> = response
        .tool_calls
        .iter()
        .map(|c| c.arguments.as_str())
        .collect();
    assert_eq!(arguments, ["{}", "{}"]);
}

// spec: TCL-14 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn calls_without_an_id_get_distinct_ones() {
    let response = answer_to(
        r#"{"role":"assistant","tool_calls":[
            {"type":"function","function":{"name":"t","arguments":"{}"}},
            {"id":"","type":"function","function":{"name":"t","arguments":"{}"}},
            {"id":"call_0","type":"function","function":{"name":"t","arguments":"{}"}}]}"#,
    )
    .await
    .unwrap();
    let ids: std::collections::BTreeSet<_> =
        response.tool_calls.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert!(!ids.contains(""), "{ids:?}");
}

/// An answer holding one call to `list_domains` and the reasoning fields `fields` (a JSON fragment
/// starting with a comma, or empty).
fn reasoned_answer(fields: &str) -> String {
    format!(
        r#"{{"model":"m","choices":[{{"finish_reason":"tool_calls","message":{{"role":"assistant","content":""{fields},
            "tool_calls":[{{"id":"call_1","type":"function","function":{{"name":"list_domains","arguments":"{{}}"}}}}]}}}}]}}"#
    )
}

fn reasoned_request() -> CompletionRequest {
    CompletionRequest {
        tools: vec![list_domains()],
        ..request()
    }
}

// spec: TCL-17 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn the_reasoning_content_of_an_answer_reaches_the_caller_with_its_calls() {
    let (_, response) = exchange(
        reasoned_request(),
        &reasoned_answer(r#","reasoning_content":"look up the domains""#),
    )
    .await;

    let reasoning = response.reasoning.unwrap();
    assert_eq!(
        reasoning.reasoning_content.as_deref(),
        Some("look up the domains")
    );
    assert_eq!((reasoning.reasoning, reasoning.details), (None, None));
    assert_eq!(response.tool_calls.len(), 1);
}

// spec: TCL-17 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn the_reasoning_of_an_ollama_answer_reaches_the_caller_under_its_field() {
    let (_, response) = exchange(
        reasoned_request(),
        &reasoned_answer(r#","reasoning":"look up the domains""#),
    )
    .await;

    let reasoning = response.reasoning.unwrap();
    assert_eq!(reasoning.reasoning.as_deref(), Some("look up the domains"));
    assert_eq!(
        (reasoning.reasoning_content, reasoning.details),
        (None, None)
    );
}

// spec: TCL-17, TCL-20 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn the_reasoning_details_of_an_openrouter_answer_reach_the_caller_as_sent() {
    let (_, response) = exchange(
        reasoned_request(),
        &reasoned_answer(
            r#","reasoning":"look up","reasoning_details":[
                {"type":"reasoning.encrypted","data":"x","id":"r1","format":"f","index":0}]"#,
        ),
    )
    .await;

    let reasoning = response.reasoning.unwrap();
    assert_eq!(reasoning.reasoning.as_deref(), Some("look up"));
    assert_eq!(
        reasoning.details,
        Some(serde_json::json!([
            {"type": "reasoning.encrypted", "data": "x", "id": "r1", "format": "f", "index": 0}
        ]))
    );
    assert_eq!(reasoning.reasoning_content, None);
}

// spec: TCL-17 (internal: the answer is served by the mock server of the provider's tests)
#[tokio::test]
async fn an_empty_reasoning_is_kept_and_a_missing_or_null_one_is_none() {
    let answer = |reasoning: &str| {
        format!(
            r#"{{"model":"m","choices":[{{"message":{{"role":"assistant","content":"ok"{reasoning}}}}}]}}"#
        )
    };
    let (_, empty) = exchange(request(), &answer(r#","reasoning_content":"""#)).await;
    let (_, missing) = exchange(request(), &answer("")).await;
    let (_, null) = exchange(
        request(),
        &answer(r#","reasoning":null,"reasoning_details":null"#),
    )
    .await;
    assert_eq!(
        empty.reasoning.and_then(|r| r.reasoning_content).as_deref(),
        Some("")
    );
    assert_eq!(missing.reasoning, None);
    assert_eq!(null.reasoning, None);
}

// spec: TCL-18, TCL-20 (internal: the wire body is only visible to the mock server of the provider's tests)
#[tokio::test]
async fn each_reasoning_field_is_sent_back_only_when_the_server_sent_it() {
    let (_, answered) = exchange(
        reasoned_request(),
        &reasoned_answer(
            r#","reasoning":"look up","reasoning_details":[{"type":"reasoning.encrypted","data":"x"}]"#,
        ),
    )
    .await;
    let call = answered.tool_calls[0].clone();
    let (body, _) = exchange(
        CompletionRequest {
            messages: vec![
                ChatMessage::new(Role::User, "Which domains?"),
                ChatMessage::assistant_calls("", vec![call.clone()])
                    .with_reasoning(answered.reasoning),
                ChatMessage::tool_result("call_1", "- Billing"),
                ChatMessage::assistant_calls("", vec![call.clone()]).with_reasoning(Some(
                    Reasoning {
                        reasoning_content: Some("look up the domains".to_string()),
                        ..Reasoning::default()
                    },
                )),
                ChatMessage::assistant_calls("", vec![call]),
            ],
            ..reasoned_request()
        },
        OK_BODY,
    )
    .await;

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[1]["reasoning"], "look up");
    assert_eq!(
        messages[1]["reasoning_details"],
        serde_json::json!([{"type": "reasoning.encrypted", "data": "x"}])
    );
    assert!(
        messages[1].get("reasoning_content").is_none(),
        "{}",
        messages[1]
    );
    assert_eq!(messages[3]["reasoning_content"], "look up the domains");
    for field in ["reasoning", "reasoning_details"] {
        assert!(messages[3].get(field).is_none(), "{}", messages[3]);
    }
    for message in [&messages[0], &messages[2], &messages[4]] {
        for field in ["reasoning_content", "reasoning", "reasoning_details"] {
            assert!(message.get(field).is_none(), "{message}");
        }
    }
}
