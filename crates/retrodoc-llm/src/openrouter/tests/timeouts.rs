use std::time::Duration;

use retrodoc_core::config::LlmConfig;

use super::super::{request_timeout, OpenRouterProvider};
use super::{provider, request};
use crate::{LlmError, LlmProvider};

#[tokio::test]
// spec: LTO-4 (internal: a timeout shows through the binary only against a server that hangs)
async fn a_timeout_is_named_and_not_retried() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, accepted) = std::sync::mpsc::channel();
    // Takes the connections, reports each peer and never answers.
    std::thread::spawn(move || {
        let mut held = Vec::new();
        while let Ok((stream, peer)) = listener.accept() {
            let _ = tx.send(peer);
            held.push(stream);
        }
    });
    let provider = OpenRouterProvider {
        client: reqwest::Client::builder()
            .timeout(Duration::from_millis(200))
            .build()
            .unwrap(),
        timeout: Duration::from_millis(200),
        ..provider(&format!("http://{addr}"))
    };

    let error = provider.complete(request()).await.unwrap_err();

    assert!(
        matches!(&error, LlmError::Transport(m) if m.contains("timed out after 200ms") && m.contains("llm.timeout_secs")),
        "{error:?}"
    );
    // The listener accepts in arrival order: every connection of the provider
    // is accepted before this sentinel, opened once it gave up.
    let sentinel = std::net::TcpStream::connect(addr).unwrap();
    let sentinel = sentinel.local_addr().unwrap();
    let before_sentinel = std::iter::from_fn(|| accepted.recv_timeout(Duration::from_secs(5)).ok())
        .take_while(|peer| *peer != sentinel)
        .count();
    assert_eq!(before_sentinel, 1);
}

fn timeout_for(base_url: Option<&str>) -> Duration {
    request_timeout(&LlmConfig {
        base_url: base_url.map(str::to_string),
        ..LlmConfig::default()
    })
}

#[test]
// spec: LTO-1 (internal: a timeout shows through the binary only against a server that hangs)
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
// spec: LTO-2 (internal: a timeout shows through the binary only against a server that hangs)
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
// spec: LTO-3 (internal: a timeout shows through the binary only against a server that hangs)
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
