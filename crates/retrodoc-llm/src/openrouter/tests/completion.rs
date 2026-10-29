use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::super::{OpenRouterProvider, DEFAULT_REQUEST_TIMEOUT, MAX_COMPLETION_TOKENS};
use super::request;
use crate::{ChatMessage, CompletionRequest, CompletionResponse, LlmProvider, Role, Usage};

#[tokio::test]
async fn complete_parses_a_successful_response() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let payload = r#"{"model":"anthropic/claude-sonnet-4.5","choices":[{"message":{"role":"assistant","content":"hello"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let provider = OpenRouterProvider {
        client: reqwest::Client::new(),
        api_key: "test-key".to_string(),
        default_model: "anthropic/claude-sonnet-4.5".to_string(),
        endpoint: String::new(),
        reasoning_effort: None,
        temperature: None,
        structured_output: true,
        require_parameters: false,
        schema_refused: AtomicBool::new(false),
        temperature_refused: AtomicBool::new(false),
        timeout: DEFAULT_REQUEST_TIMEOUT,
    }
    .with_endpoint(format!("http://{addr}"));

    let response = provider
        .complete(CompletionRequest {
            messages: vec![ChatMessage {
                role: Role::User,
                content: "hi".to_string(),
            }],
            model: None,
            json_schema: None,
        })
        .await
        .unwrap();

    assert_eq!(response.content, "hello");
    assert_eq!(response.model, "anthropic/claude-sonnet-4.5");
    // The server sent no `usage`: nothing is invented.
    assert_eq!(response.usage, None);
}

/// Serves `payload` once and returns the provider's answer.
async fn complete_with_payload(payload: &'static str) -> CompletionResponse {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    let provider = OpenRouterProvider {
        client: reqwest::Client::new(),
        api_key: "test-key".to_string(),
        default_model: "m".to_string(),
        endpoint: String::new(),
        reasoning_effort: None,
        temperature: None,
        structured_output: true,
        require_parameters: false,
        schema_refused: AtomicBool::new(false),
        temperature_refused: AtomicBool::new(false),
        timeout: DEFAULT_REQUEST_TIMEOUT,
    }
    .with_endpoint(format!("http://{addr}"));
    provider.complete(request()).await.unwrap()
}

#[tokio::test]
async fn an_answer_cut_by_the_completion_limit_is_flagged_and_a_complete_one_is_not() {
    let cut = complete_with_payload(
        r#"{"model":"m","choices":[{"finish_reason":"length","message":{"role":"assistant","content":"{\"rules\":[{"}}]}"#,
    )
    .await;
    assert!(cut.truncated);
    let whole = complete_with_payload(
        r#"{"model":"m","choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"ok"}}]}"#,
    )
    .await;
    assert!(!whole.truncated);
}

#[tokio::test]
async fn complete_reads_the_usage_the_server_reports() {
    let response = complete_with_payload(
        r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"ok"}}],"usage":{"prompt_tokens":120,"completion_tokens":35,"total_tokens":155}}"#,
    )
    .await;
    assert_eq!(
        response.usage,
        Some(Usage {
            prompt_tokens: 120,
            completion_tokens: 35,
            cached_tokens: 0
        })
    );
}

#[tokio::test]
async fn a_malformed_usage_does_not_cost_the_answer() {
    let response = complete_with_payload(
        r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"ok"}}],"usage":{"prompt_tokens":12.5,"completion_tokens":-3}}"#,
    )
    .await;
    assert_eq!(response.content, "ok");
    assert_eq!(response.usage, None);
}

#[tokio::test]
async fn complete_waits_for_the_delay_the_server_asks_for_on_a_429() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for round in 0..2 {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let response = if round == 0 {
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            } else {
                let payload =
                    r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                )
            };
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let provider = OpenRouterProvider {
        client: reqwest::Client::new(),
        api_key: "test-key".to_string(),
        default_model: "m".to_string(),
        endpoint: String::new(),
        reasoning_effort: None,
        temperature: None,
        structured_output: true,
        require_parameters: false,
        schema_refused: AtomicBool::new(false),
        temperature_refused: AtomicBool::new(false),
        timeout: DEFAULT_REQUEST_TIMEOUT,
    }
    .with_endpoint(format!("http://{addr}"));

    let started = std::time::Instant::now();
    let response = provider
        .complete(CompletionRequest {
            messages: vec![ChatMessage {
                role: Role::User,
                content: "hi".to_string(),
            }],
            model: None,
            json_schema: None,
        })
        .await
        .unwrap();

    assert_eq!(response.content, "ok");
    // Retry-After: 1 plus the one-second margin, well above the 500 ms backoff.
    assert!(started.elapsed() >= Duration::from_secs(2));
}

#[tokio::test]
async fn complete_caps_max_tokens_to_guard_against_a_runaway_local_model() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let _ = tx.send(String::from_utf8_lossy(&buf[..n]).to_string());
            let payload =
                r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let provider = OpenRouterProvider {
        client: reqwest::Client::new(),
        api_key: "test-key".to_string(),
        default_model: "m".to_string(),
        endpoint: String::new(),
        reasoning_effort: None,
        temperature: None,
        structured_output: true,
        require_parameters: false,
        schema_refused: AtomicBool::new(false),
        temperature_refused: AtomicBool::new(false),
        timeout: DEFAULT_REQUEST_TIMEOUT,
    }
    .with_endpoint(format!("http://{addr}"));

    provider
        .complete(CompletionRequest {
            messages: vec![ChatMessage {
                role: Role::User,
                content: "hi".to_string(),
            }],
            model: None,
            json_schema: None,
        })
        .await
        .unwrap();

    let request = rx.recv().unwrap();
    assert!(request.contains(&format!("\"max_tokens\":{MAX_COMPLETION_TOKENS}")));
}

#[tokio::test]
async fn reasoning_effort_is_sent_only_when_configured() {
    for (effort, expected) in [(Some("none"), true), (None, false)] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let _ = tx.send(String::from_utf8_lossy(&buf[..n]).to_string());
                let payload =
                    r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let provider = OpenRouterProvider {
            client: reqwest::Client::new(),
            api_key: "test-key".to_string(),
            default_model: "m".to_string(),
            endpoint: String::new(),
            reasoning_effort: effort.map(str::to_string),
            temperature: None,
            structured_output: true,
            require_parameters: false,
            schema_refused: AtomicBool::new(false),
            temperature_refused: AtomicBool::new(false),
            timeout: DEFAULT_REQUEST_TIMEOUT,
        }
        .with_endpoint(format!("http://{addr}"));

        provider
            .complete(CompletionRequest {
                messages: vec![ChatMessage {
                    role: Role::User,
                    content: "hi".to_string(),
                }],
                model: None,
                json_schema: None,
            })
            .await
            .unwrap();

        let request = rx.recv().unwrap();
        assert_eq!(request.contains("\"reasoning_effort\":\"none\""), expected);
    }
}
