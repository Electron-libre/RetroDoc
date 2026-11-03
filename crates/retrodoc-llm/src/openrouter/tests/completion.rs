use std::time::Duration;

use super::super::{OpenRouterProvider, MAX_COMPLETION_TOKENS};
use super::{http, provider, request, serve};
use crate::{CompletionResponse, LlmProvider, Usage};

const OK_PAYLOAD: &str =
    r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;

#[tokio::test]
async fn complete_parses_a_successful_response() {
    let response = complete_with_payload(
        r#"{"model":"anthropic/claude-sonnet-4.5","choices":[{"message":{"role":"assistant","content":"hello"}}]}"#,
    )
    .await;

    assert_eq!(response.content, "hello");
    assert_eq!(response.model, "anthropic/claude-sonnet-4.5");
    // The server sent no `usage`: nothing is invented.
    assert_eq!(response.usage, None);
}

/// Serves `payload` once and returns the provider's answer.
async fn complete_with_payload(payload: &str) -> CompletionResponse {
    let (url, _rx) = serve(vec![http("200 OK", payload)]);
    provider(&url).complete(request()).await.unwrap()
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
    let (url, _rx) = serve(vec![
        "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_string(),
        http("200 OK", OK_PAYLOAD),
    ]);

    let started = std::time::Instant::now();
    let response = provider(&url).complete(request()).await.unwrap();

    assert_eq!(response.content, "ok");
    // Retry-After: 1 plus the one-second margin, well above the 500 ms backoff.
    assert!(started.elapsed() >= Duration::from_secs(2));
}

#[tokio::test]
async fn complete_caps_max_tokens_to_guard_against_a_runaway_local_model() {
    let (url, rx) = serve(vec![http("200 OK", OK_PAYLOAD)]);

    provider(&url).complete(request()).await.unwrap();

    let request = rx.recv().unwrap();
    assert!(request.contains(&format!("\"max_tokens\":{MAX_COMPLETION_TOKENS}")));
}

#[tokio::test]
async fn reasoning_effort_is_sent_only_when_configured() {
    for (effort, expected) in [(Some("none"), true), (None, false)] {
        let (url, rx) = serve(vec![http("200 OK", OK_PAYLOAD)]);

        OpenRouterProvider {
            reasoning_effort: effort.map(str::to_string),
            ..provider(&url)
        }
        .complete(request())
        .await
        .unwrap();

        let request = rx.recv().unwrap();
        assert_eq!(request.contains("\"reasoning_effort\":\"none\""), expected);
    }
}
