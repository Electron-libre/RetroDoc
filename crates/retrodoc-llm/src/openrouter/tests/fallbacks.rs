use std::sync::atomic::AtomicBool;

use super::super::{OpenRouterProvider, DEFAULT_REQUEST_TIMEOUT};
use super::request;
use crate::{CompletionRequest, LlmProvider, ResponseSchema};

/// Serves the given raw HTTP responses in order, one per connection, and
/// returns the requests received.
fn serve(responses: Vec<String>) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for payload in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let _ = tx.send(String::from_utf8_lossy(&buf[..n]).to_string());
            let _ = stream.write_all(payload.as_bytes());
        }
    });
    (format!("http://{addr}"), rx)
}

fn http(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

const OK_BODY: &str =
    r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"{}"}}]}"#;

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
        client: reqwest::Client::new(),
        api_key: "test-key".to_string(),
        default_model: "m".to_string(),
        endpoint: String::new(),
        reasoning_effort: None,
        temperature: None,
        structured_output,
        require_parameters: true,
        schema_refused: AtomicBool::new(false),
        temperature_refused: AtomicBool::new(false),
        timeout: DEFAULT_REQUEST_TIMEOUT,
    }
    .with_endpoint(endpoint)
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
