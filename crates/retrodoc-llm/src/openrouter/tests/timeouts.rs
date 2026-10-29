use std::sync::atomic::AtomicBool;
use std::time::Duration;

use retrodoc_core::config::LlmConfig;

use super::super::{request_timeout, OpenRouterProvider};
use super::request;
use crate::{LlmError, LlmProvider};

#[tokio::test]
// spec: LTO-4
async fn a_timeout_is_named_and_not_retried() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = accepted.clone();
    // Takes the connections and never answers.
    std::thread::spawn(move || {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept() {
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            held.push(stream);
        }
    });
    let provider = OpenRouterProvider {
        client: reqwest::Client::builder()
            .timeout(Duration::from_millis(200))
            .build()
            .unwrap(),
        api_key: "test-key".to_string(),
        default_model: "m".to_string(),
        endpoint: String::new(),
        reasoning_effort: None,
        temperature: None,
        structured_output: true,
        require_parameters: false,
        schema_refused: AtomicBool::new(false),
        temperature_refused: AtomicBool::new(false),
        timeout: Duration::from_millis(200),
    }
    .with_endpoint(format!("http://{addr}"));

    let error = provider.complete(request()).await.unwrap_err();

    assert!(
        matches!(&error, LlmError::Transport(m) if m.contains("timed out after 200ms") && m.contains("llm.timeout_secs")),
        "{error:?}"
    );
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);
}

fn timeout_for(base_url: Option<&str>) -> Duration {
    request_timeout(&LlmConfig {
        base_url: base_url.map(str::to_string),
        ..LlmConfig::default()
    })
}

#[test]
// spec: LTO-1
fn a_loopback_server_defaults_to_fifteen_minutes() {
    for url in [
        "http://localhost:11434/v1/chat/completions",
        "http://127.0.0.1:11437/v1/chat/completions",
        "http://127.3.4.5/v1",
        "http://[::1]:11435/v1/chat/completions",
        "HTTP://LOCALHOST/v1",
    ] {
        assert_eq!(timeout_for(Some(url)), Duration::from_mins(15), "{url}");
    }
}

#[test]
// spec: LTO-2
fn a_hosted_endpoint_defaults_to_two_minutes() {
    for url in [
        None,
        Some("https://openrouter.ai/api/v1/chat/completions"),
        Some("https://localhost.example.com/v1"),
        Some("http://192.168.1.20:11434/v1"),
        Some("not a url"),
    ] {
        assert_eq!(timeout_for(url), Duration::from_secs(120), "{url:?}");
    }
}

#[test]
// spec: LTO-3
fn a_configured_timeout_wins_over_both_defaults() {
    let mut config = LlmConfig {
        base_url: Some("http://127.0.0.1:11437/v1".to_string()),
        timeout_secs: Some(60),
        ..LlmConfig::default()
    };
    assert_eq!(request_timeout(&config), Duration::from_secs(60));
    config.base_url = None;
    config.timeout_secs = Some(600);
    assert_eq!(request_timeout(&config), Duration::from_secs(600));
    config.timeout_secs = None;
    config.passes.insert(
        "use-cases".to_string(),
        retrodoc_core::config::PassLlmConfig {
            timeout_secs: Some(30),
            ..Default::default()
        },
    );
    assert_eq!(
        request_timeout(&config.for_pass("use-cases")),
        Duration::from_secs(30)
    );
}
