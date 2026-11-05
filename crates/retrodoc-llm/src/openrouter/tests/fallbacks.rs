use super::super::OpenRouterProvider;
use super::{http, provider, request, serve, OK_BODY};
use crate::{CompletionRequest, LlmProvider, ResponseSchema};

fn schema_request() -> CompletionRequest {
    CompletionRequest {
        json_schema: Some(ResponseSchema {
            name: "answer".to_string(),
            schema: serde_json::json!({"type": "object"}),
        }),
        ..request()
    }
}

fn schema_provider(endpoint: &str, structured_output: bool) -> OpenRouterProvider {
    OpenRouterProvider {
        structured_output,
        require_parameters: true,
        ..provider(endpoint)
    }
}

#[tokio::test]
async fn the_schema_is_sent_as_a_strict_response_format() {
    let (url, rx) = serve(vec![http("200 OK", OK_BODY)]);
    schema_provider(&url, true)
        .complete(schema_request())
        .await
        .unwrap();
    let sent = rx.recv().unwrap();
    assert!(sent.contains("\"response_format\":{\"type\":\"json_schema\""));
    assert!(sent.contains("\"name\":\"answer\",\"strict\":true"));
    assert!(sent.contains("\"require_parameters\":true"));
}

#[tokio::test]
async fn no_schema_is_sent_when_the_setting_is_off_or_the_request_has_none() {
    for (structured_output, request) in [(false, schema_request()), (true, request())] {
        let (url, rx) = serve(vec![http("200 OK", OK_BODY)]);
        schema_provider(&url, structured_output)
            .complete(request)
            .await
            .unwrap();
        let sent = rx.recv().unwrap();
        assert!(!sent.contains("response_format"));
        assert!(!sent.contains("require_parameters"));
    }
}

#[tokio::test]
async fn a_refused_schema_falls_back_to_the_plain_request_and_is_remembered() {
    let (url, rx) = serve(vec![
        http(
            "400 Bad Request",
            r#"{"error":"unknown field response_format"}"#,
        ),
        http("200 OK", OK_BODY),
        http("200 OK", OK_BODY),
    ]);
    let provider = schema_provider(&url, true);
    provider.complete(schema_request()).await.unwrap();
    provider.complete(schema_request()).await.unwrap();
    let sent: Vec<String> = rx.try_iter().collect();
    assert_eq!(sent.len(), 3);
    assert!(sent[0].contains("response_format"));
    assert!(!sent[1].contains("response_format"));
    assert!(!sent[2].contains("response_format"));
}

#[tokio::test]
async fn a_client_error_that_the_plain_request_shares_is_not_blamed_on_the_schema() {
    let refusal = http("400 Bad Request", r#"{"error":"prompt too long"}"#);
    let (url, rx) = serve(vec![refusal.clone(), refusal, http("200 OK", OK_BODY)]);
    let provider = schema_provider(&url, true);
    let err = provider.complete(schema_request()).await.unwrap_err();
    assert!(err.to_string().contains("prompt too long"));
    provider.complete(schema_request()).await.unwrap();
    let sent: Vec<String> = rx.try_iter().collect();
    assert!(sent[2].contains("response_format"));
}

fn temperature_provider(endpoint: &str, temperature: Option<f32>) -> OpenRouterProvider {
    OpenRouterProvider {
        temperature,
        ..schema_provider(endpoint, true)
    }
}

#[tokio::test]
async fn the_temperature_is_sent_only_when_configured() {
    for (temperature, expected) in [(Some(0.0), true), (None, false)] {
        let (url, rx) = serve(vec![http("200 OK", OK_BODY)]);
        temperature_provider(&url, temperature)
            .complete(request())
            .await
            .unwrap();
        assert_eq!(rx.recv().unwrap().contains("\"temperature\":0.0"), expected);
    }
}

#[tokio::test]
async fn a_refused_temperature_falls_back_keeping_the_schema_and_is_remembered() {
    let (url, rx) = serve(vec![
        http(
            "400 Bad Request",
            r#"{"error":"unknown field temperature"}"#,
        ),
        http("200 OK", OK_BODY),
        http("200 OK", OK_BODY),
    ]);
    let provider = temperature_provider(&url, Some(0.0));
    provider.complete(schema_request()).await.unwrap();
    provider.complete(schema_request()).await.unwrap();
    let sent: Vec<String> = rx.try_iter().collect();
    assert_eq!(sent.len(), 3);
    assert!(sent[0].contains("temperature"));
    for plain in &sent[1..] {
        assert!(!plain.contains("temperature"));
        assert!(plain.contains("response_format"));
    }
}

#[tokio::test]
async fn a_refused_schema_keeps_the_temperature() {
    let (url, rx) = serve(vec![
        http("400 Bad Request", r#"{"error":"no schema"}"#),
        http("400 Bad Request", r#"{"error":"no schema"}"#),
        http("200 OK", OK_BODY),
    ]);
    temperature_provider(&url, Some(0.0))
        .complete(schema_request())
        .await
        .unwrap();
    let sent: Vec<String> = rx.try_iter().collect();
    assert_eq!(sent.len(), 3);
    assert!(!sent[2].contains("response_format"));
    assert!(sent[2].contains("\"temperature\":0.0"));
}

#[tokio::test]
async fn a_server_refusing_both_the_temperature_and_the_schema_gets_the_plain_request() {
    let refusal = http("400 Bad Request", r#"{"error":"unknown parameter"}"#);
    let (url, rx) = serve(vec![
        refusal.clone(),
        refusal.clone(),
        refusal,
        http("200 OK", OK_BODY),
    ]);
    temperature_provider(&url, Some(0.0))
        .complete(schema_request())
        .await
        .unwrap();
    let sent: Vec<String> = rx.try_iter().collect();
    assert_eq!(sent.len(), 4);
    assert!(!sent[3].contains("temperature") && !sent[3].contains("response_format"));
}

fn tools_request() -> CompletionRequest {
    CompletionRequest {
        tools: vec![crate::ToolSpec {
            name: "list_domains".to_string(),
            description: "The domains.".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        }],
        ..schema_request()
    }
}

// spec: TCL-15, TCL-16 (internal: the refusal is served by the mock server of the provider's tests)
#[tokio::test]
async fn a_refusal_of_the_tools_fails_at_once_naming_the_tools_and_the_model() {
    let refusals = [
        (
            "400 Bad Request",
            r#"{"error":"registry.ollama.ai/library/m does not support tools"}"#,
        ),
        (
            "404 Not Found",
            r#"{"error":{"message":"No endpoints found that support tool use."}}"#,
        ),
    ];
    for (status, body) in refusals {
        let (url, rx) = serve(vec![
            http(status, body),
            http("200 OK", OK_BODY),
            http("200 OK", OK_BODY),
            http("200 OK", OK_BODY),
        ]);
        let err = temperature_provider(&url, Some(0.0))
            .complete(tools_request())
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(rx.try_iter().count(), 1, "{status}: no fallback");
        assert!(err.contains("refuses tools"), "{err}");
        assert!(err.contains("`m`"), "{err}");
        assert!(err.contains("support tool"), "{err}");
    }
}

// spec: TCL-15 (internal: the refusal is served by the mock server of the provider's tests)
#[tokio::test]
async fn a_refused_temperature_falls_back_on_a_request_offering_tools() {
    let (url, rx) = serve(vec![
        http(
            "400 Bad Request",
            r#"{"error":"unknown field temperature"}"#,
        ),
        http("200 OK", OK_BODY),
    ]);
    temperature_provider(&url, Some(0.0))
        .complete(tools_request())
        .await
        .unwrap();
    let sent: Vec<String> = rx.try_iter().collect();
    assert_eq!(sent.len(), 2);
    assert!(!sent[1].contains("temperature"));
}

// spec: TCL-15 (internal: the refusal is served by the mock server of the provider's tests)
#[tokio::test]
async fn a_refusal_of_the_schema_with_the_tools_falls_back_without_the_schema() {
    let (url, rx) = serve(vec![
        http(
            "400 Bad Request",
            r#"{"error":"response_format is not supported together with tools"}"#,
        ),
        http("200 OK", OK_BODY),
    ]);
    provider(&url).complete(tools_request()).await.unwrap();
    let sent: Vec<String> = rx.try_iter().collect();
    assert_eq!(sent.len(), 2);
    assert!(!sent[1].contains("response_format"));
}
