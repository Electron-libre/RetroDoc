//! `retrodoc-llm`: LLM provider abstraction (`LlmProvider`), so `RetroDoc`
//! isn't locked to `OpenRouter` (PLAN.md §1). Only `OpenRouter` is
//! implemented in v1. The real HTTP call (PLAN.md §4: "retry, rate-limit")
//! is wired up here, at the "repo map" phase — the first pipeline step to
//! consume an LLM.

use std::time::Duration;

use async_trait::async_trait;
use retrodoc_core::config::LlmConfig;
use serde::{Deserialize, Serialize};

const OPENROUTER_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
/// Number of extra attempts after the initial call, on transient errors
/// (429 / 5xx) — PLAN.md §4 "retry, rate-limit".
const MAX_RETRIES: u32 = 3;
const RETRY_BASE_DELAY: Duration = Duration::from_millis(500);
/// Longest wait the provider may ask for (`Retry-After`, `retryDelay`) that
/// is still honored; beyond it the error is returned (e.g. a daily quota).
const MAX_SERVER_RETRY_DELAY: Duration = Duration::from_secs(120);
/// Default per-request HTTP timeout (`llm.timeout_secs` overrides it). `OpenRouter` always responds well within this,
/// but a local `llm.base_url` override (a small quantized model) can
/// degenerate into a runaway generation loop that never reaches a stop
/// token; without a client-side bound the call hangs indefinitely instead
/// of surfacing an error the existing retry loop can act on.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
/// Upper bound on `max_tokens` sent with every request, for the same
/// reason: caps how long a runaway generation can run server-side too.
const MAX_COMPLETION_TOKENS: u32 = 8192;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    fn as_api_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<ChatMessage>,
    /// Model to use; overrides the one from the config if present.
    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub content: String,
    pub model: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("environment variable {0} is not set (LLM API key)")]
    MissingApiKey(String),
    #[error("network call to the LLM provider failed: {0}")]
    Transport(String),
    #[error("invalid response from the LLM provider: {0}")]
    InvalidResponse(String),
    #[error("LLM provider \"{0}\" is not supported in v1 (only openrouter is)")]
    UnsupportedProvider(String),
}

/// Contract shared by every LLM provider, so other providers than
/// `OpenRouter` can be added without touching the rest of the pipeline.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;
}

/// Interval of the "still waiting" log of [`HeartbeatProvider::new`].
pub const DEFAULT_HEARTBEAT: Duration = Duration::from_secs(30);

/// Wraps a provider to report, at a regular interval, that a call is still
/// pending (including the retries and backoff inside it), so a slow call can
/// be told from a stuck run in the logs.
pub struct HeartbeatProvider<P> {
    inner: P,
    interval: Duration,
    report: Box<dyn Fn(Duration) + Send + Sync>,
}

impl<P> HeartbeatProvider<P> {
    /// Logs "still waiting for the LLM" every [`DEFAULT_HEARTBEAT`].
    #[must_use]
    pub fn new(inner: P) -> Self {
        Self::with_reporter(inner, DEFAULT_HEARTBEAT, |waited| {
            tracing::info!("still waiting for the LLM ({}s)", waited.as_secs());
        })
    }

    /// `report` is called every `interval` with the time waited so far.
    #[must_use]
    pub fn with_reporter(
        inner: P,
        interval: Duration,
        report: impl Fn(Duration) + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner,
            interval,
            report: Box::new(report),
        }
    }
}

#[async_trait]
impl<P: LlmProvider> LlmProvider for HeartbeatProvider<P> {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let started = tokio::time::Instant::now();
        let call = self.inner.complete(request);
        tokio::pin!(call);
        let mut ticker = tokio::time::interval_at(started + self.interval, self.interval);
        loop {
            tokio::select! {
                result = &mut call => return result,
                _ = ticker.tick() => (self.report)(started.elapsed()),
            }
        }
    }
}

// --- OpenRouter API formats (OpenAI-compatible chat completions) ---

#[derive(Debug, Serialize)]
struct ApiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct ApiRequest<'a> {
    model: &'a str,
    messages: Vec<ApiMessage<'a>>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(default)]
    model: String,
    choices: Vec<ApiChoice>,
}

#[derive(Debug, Deserialize)]
struct ApiChoice {
    message: ApiResponseMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiResponseMessage {
    content: String,
}

/// `OpenRouter` provider (<https://openrouter.ai>).
pub struct OpenRouterProvider {
    client: reqwest::Client,
    api_key: String,
    default_model: String,
    endpoint: String,
    reasoning_effort: Option<String>,
}

impl OpenRouterProvider {
    /// Builds the provider from the config, reading the API key from the
    /// environment variable named by `llm.api_key_env`. Calls `OpenRouter`
    /// unless `llm.base_url` overrides the endpoint (e.g. to point at a
    /// local OpenAI-compatible server instead).
    ///
    /// # Errors
    ///
    /// Returns an error if `config.provider` isn't `"openrouter"`, if the
    /// API key's environment variable is not set, or if the underlying
    /// HTTP client can't be built.
    pub fn from_config(config: &LlmConfig) -> Result<Self, LlmError> {
        if config.provider != "openrouter" {
            return Err(LlmError::UnsupportedProvider(config.provider.clone()));
        }
        let api_key = std::env::var(&config.api_key_env)
            .map_err(|_| LlmError::MissingApiKey(config.api_key_env.clone()))?;
        let endpoint = config
            .base_url
            .clone()
            .unwrap_or_else(|| OPENROUTER_ENDPOINT.to_string());
        let client = reqwest::Client::builder()
            .timeout(request_timeout(config))
            .build()
            .map_err(|source| LlmError::Transport(source.to_string()))?;
        Ok(Self {
            client,
            api_key,
            default_model: config.model.clone(),
            endpoint,
            reasoning_effort: config.reasoning_effort.clone(),
        })
    }

    #[cfg(test)]
    fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }
}

/// Per-request timeout: `llm.timeout_secs`, else [`DEFAULT_REQUEST_TIMEOUT`].
fn request_timeout(config: &LlmConfig) -> Duration {
    config
        .timeout_secs
        .map_or(DEFAULT_REQUEST_TIMEOUT, Duration::from_secs)
}

/// How long the provider asks to wait before retrying: the `Retry-After`
/// header (seconds), else the `retryDelay` field of a Google-style error body
/// (`"retryDelay": "43s"`). A second is added so the retry lands after the
/// quota window rather than on its edge.
fn server_retry_delay(retry_after: Option<&str>, body: &str) -> Option<Duration> {
    let seconds = retry_after
        .and_then(|v| v.trim().parse::<f64>().ok())
        .or_else(|| {
            let rest = &body[body.find("retryDelay")? + "retryDelay".len()..];
            let rest = &rest[rest.find(|c: char| c.is_ascii_digit())?..];
            let number: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            rest[number.len()..]
                .starts_with('s')
                .then(|| number.parse::<f64>().ok())?
        })?;
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds + 1.0))
}

/// An HTTP status deserves a retry if it signals a transient error on the
/// provider's side: rate-limit (429) or server error (5xx).
fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let model = request.model.as_deref().unwrap_or(&self.default_model);
        let messages: Vec<ApiMessage> = request
            .messages
            .iter()
            .map(|m| ApiMessage {
                role: m.role.as_api_str(),
                content: &m.content,
            })
            .collect();
        let body = ApiRequest {
            model,
            messages,
            max_tokens: MAX_COMPLETION_TOKENS,
            reasoning_effort: self.reasoning_effort.as_deref(),
        };

        let mut attempt = 0;
        loop {
            let result = self
                .client
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let parsed: ApiResponse = response.json().await.map_err(|source| {
                            LlmError::InvalidResponse(format!("unreadable response body: {source}"))
                        })?;
                        let choice = parsed.choices.into_iter().next().ok_or_else(|| {
                            LlmError::InvalidResponse("no choice in the response".to_string())
                        })?;
                        if choice.finish_reason.as_deref() == Some("length") {
                            // Cut off by the token limit: either our
                            // `max_tokens` cap or, on a local server, its
                            // context window (Ollama defaults to 4096
                            // tokens, prompt included). The JSON the passes
                            // expect will then be incomplete.
                            tracing::warn!(
                                model,
                                "response truncated (finish_reason=length): raise the server's \
                                 context length or shrink the prompt"
                            );
                        }
                        let content = choice.message.content;
                        let model = if parsed.model.is_empty() {
                            model.to_string()
                        } else {
                            parsed.model
                        };
                        return Ok(CompletionResponse { content, model });
                    }

                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string);
                    let text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "<unreadable body>".to_string());

                    if is_retryable_status(status) && attempt < MAX_RETRIES {
                        let server_delay = server_retry_delay(retry_after.as_deref(), &text);
                        // A wait too long to be worth it (a daily quota, say)
                        // is an error, not a pause.
                        if server_delay.is_none_or(|d| d <= MAX_SERVER_RETRY_DELAY) {
                            attempt += 1;
                            let backoff = RETRY_BASE_DELAY * 2u32.pow(attempt - 1);
                            let delay = server_delay.map_or(backoff, |d| d.max(backoff));
                            tracing::warn!(
                                status = %status,
                                attempt,
                                server_requested = server_delay.is_some(),
                                "transient OpenRouter response, retrying in {delay:?}"
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                    }

                    return Err(LlmError::InvalidResponse(format!(
                        "HTTP status {status}: {text}"
                    )));
                }
                Err(source) => {
                    if attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = RETRY_BASE_DELAY * 2u32.pow(attempt - 1);
                        tracing::warn!(
                            error = %source,
                            attempt,
                            "OpenRouter network failure, retrying in {delay:?}"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(LlmError::Transport(source.to_string()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SlowProvider(Duration);

    #[async_trait]
    impl LlmProvider for SlowProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            tokio::time::sleep(self.0).await;
            Ok(CompletionResponse {
                content: "done".to_string(),
                model: "m".to_string(),
            })
        }
    }

    fn request() -> CompletionRequest {
        CompletionRequest {
            messages: Vec::new(),
            model: None,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn heartbeat_reports_while_a_call_is_pending_and_passes_the_answer_through() {
        let ticks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = ticks.clone();
        let provider = HeartbeatProvider::with_reporter(
            SlowProvider(Duration::from_secs(100)),
            Duration::from_secs(30),
            move |waited| seen.lock().unwrap().push(waited.as_secs()),
        );

        let response = provider.complete(request()).await.unwrap();

        assert_eq!(response.content, "done");
        assert_eq!(*ticks.lock().unwrap(), vec![30, 60, 90]);
    }

    #[tokio::test(start_paused = true)]
    async fn heartbeat_is_silent_for_a_fast_call() {
        let ticks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = ticks.clone();
        let provider = HeartbeatProvider::with_reporter(
            SlowProvider(Duration::from_secs(5)),
            Duration::from_secs(30),
            move |waited| seen.lock().unwrap().push(waited.as_secs()),
        );
        provider.complete(request()).await.unwrap();
        assert!(ticks.lock().unwrap().is_empty());
    }

    #[test]
    fn missing_api_key_env_is_reported() {
        let config = LlmConfig {
            provider: "openrouter".to_string(),
            api_key_env: "RETRODOC_TEST_MISSING_KEY_VAR".to_string(),
            model: "anthropic/claude-sonnet-4.5".to_string(),
            base_url: None,
            reasoning_effort: None,
            timeout_secs: None,
            concurrency: None,
            batch_chars: None,
        };
        std::env::remove_var(&config.api_key_env);
        let result = OpenRouterProvider::from_config(&config);
        assert!(matches!(result, Err(LlmError::MissingApiKey(_))));
    }

    #[test]
    fn unsupported_provider_is_rejected() {
        let config = LlmConfig {
            provider: "openai".to_string(),
            api_key_env: "X".to_string(),
            model: "gpt-4o".to_string(),
            base_url: None,
            reasoning_effort: None,
            timeout_secs: None,
            concurrency: None,
            batch_chars: None,
        };
        let result = OpenRouterProvider::from_config(&config);
        assert!(matches!(result, Err(LlmError::UnsupportedProvider(p)) if p == "openai"));
    }

    #[test]
    fn base_url_override_is_used_instead_of_openrouters_endpoint() {
        let config = LlmConfig {
            provider: "openrouter".to_string(),
            api_key_env: "RETRODOC_TEST_BASE_URL_KEY_VAR".to_string(),
            model: "llama3.2:3b".to_string(),
            base_url: Some("http://localhost:11434/v1/chat/completions".to_string()),
            reasoning_effort: None,
            timeout_secs: None,
            concurrency: None,
            batch_chars: None,
        };
        std::env::set_var(&config.api_key_env, "unused-for-local-servers");
        let provider = OpenRouterProvider::from_config(&config).unwrap();
        assert_eq!(
            provider.endpoint,
            "http://localhost:11434/v1/chat/completions"
        );
        std::env::remove_var(&config.api_key_env);
    }

    #[test]
    fn no_base_url_override_defaults_to_openrouter() {
        let config = LlmConfig {
            provider: "openrouter".to_string(),
            api_key_env: "RETRODOC_TEST_DEFAULT_ENDPOINT_KEY_VAR".to_string(),
            model: "anthropic/claude-sonnet-4.5".to_string(),
            base_url: None,
            reasoning_effort: None,
            timeout_secs: None,
            concurrency: None,
            batch_chars: None,
        };
        std::env::set_var(&config.api_key_env, "unused");
        let provider = OpenRouterProvider::from_config(&config).unwrap();
        assert_eq!(provider.endpoint, OPENROUTER_ENDPOINT);
        std::env::remove_var(&config.api_key_env);
    }

    #[test]
    fn server_retry_delay_reads_the_header_then_the_google_body() {
        let body = r#"{"error":{"details":[{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay": "43s"}]}}"#;
        assert_eq!(
            server_retry_delay(Some("5"), body),
            Some(Duration::from_secs(6))
        );
        assert_eq!(
            server_retry_delay(None, body),
            Some(Duration::from_secs(44))
        );
        assert_eq!(
            server_retry_delay(None, r#""retryDelay":"1.5s""#),
            Some(Duration::from_millis(2500))
        );
        assert_eq!(server_retry_delay(None, "Too Many Requests"), None);
        assert_eq!(server_retry_delay(None, r#""retryDelay":"soon""#), None);
        assert_eq!(server_retry_delay(Some("tomorrow"), "{}"), None);
    }

    #[test]
    fn retryable_statuses_are_429_and_5xx() {
        assert!(is_retryable_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(is_retryable_status(
            reqwest::StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(!is_retryable_status(reqwest::StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(reqwest::StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(reqwest::StatusCode::OK));
    }

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
        }
        .with_endpoint(format!("http://{addr}"));

        let response = provider
            .complete(CompletionRequest {
                messages: vec![ChatMessage {
                    role: Role::User,
                    content: "hi".to_string(),
                }],
                model: None,
            })
            .await
            .unwrap();

        assert_eq!(response.content, "hello");
        assert_eq!(response.model, "anthropic/claude-sonnet-4.5");
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
                    let payload = r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
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
        }
        .with_endpoint(format!("http://{addr}"));

        provider
            .complete(CompletionRequest {
                messages: vec![ChatMessage {
                    role: Role::User,
                    content: "hi".to_string(),
                }],
                model: None,
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
                    let payload = r#"{"model":"m","choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
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
            }
            .with_endpoint(format!("http://{addr}"));

            provider
                .complete(CompletionRequest {
                    messages: vec![ChatMessage {
                        role: Role::User,
                        content: "hi".to_string(),
                    }],
                    model: None,
                })
                .await
                .unwrap();

            let request = rx.recv().unwrap();
            assert_eq!(request.contains("\"reasoning_effort\":\"none\""), expected);
        }
    }

    #[test]
    fn timeout_defaults_to_two_minutes_and_follows_the_config() {
        let mut config = LlmConfig::default();
        assert_eq!(request_timeout(&config), Duration::from_secs(120));
        config.timeout_secs = Some(600);
        assert_eq!(request_timeout(&config), Duration::from_secs(600));
    }
}
