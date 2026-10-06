//! The `OpenRouter` provider (OpenAI-compatible chat completions): the HTTP
//! call, its retries and the decoding of the answer.

use std::time::Duration;

use async_trait::async_trait;
use retrodoc_core::config::LlmConfig;
use serde::{Deserialize, Serialize};

use crate::usage;
use crate::{CompletionRequest, CompletionResponse, LlmError, LlmProvider};

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
    #[serde(default)]
    usage: Option<serde_json::Value>,
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
        let body = ApiRequest {
            model,
            messages: request
                .messages
                .iter()
                .map(|m| ApiMessage {
                    role: m.role.as_api_str(),
                    content: &m.content,
                })
                .collect(),
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
            let failure = match result {
                Ok(response) if response.status().is_success() => {
                    return decode_success(response, model).await;
                }
                Ok(response) => Failure::from_response(response).await,
                Err(source) => Failure::Network(source),
            };

            if attempt >= MAX_RETRIES || !failure.is_transient() {
                return Err(failure.into_error());
            }
            attempt += 1;
            let delay = retry_delay(attempt, failure.server_delay());
            failure.warn(attempt, delay);
            tokio::time::sleep(delay).await;
        }
    }
}

/// Reads a successful response: the first choice, and the usage if reported.
async fn decode_success(
    response: reqwest::Response,
    requested_model: &str,
) -> Result<CompletionResponse, LlmError> {
    let parsed: ApiResponse = response.json().await.map_err(|source| {
        LlmError::InvalidResponse(format!("unreadable response body: {source}"))
    })?;
    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| LlmError::InvalidResponse("no choice in the response".to_string()))?;
    if choice.finish_reason.as_deref() == Some("length") {
        // Cut off by the token limit: either our `max_tokens` cap or, on a
        // local server, its context window (Ollama defaults to 4096 tokens,
        // prompt included). The JSON the passes expect will then be
        // incomplete.
        tracing::warn!(
            model = requested_model,
            "response truncated (finish_reason=length): raise the server's \
             context length or shrink the prompt"
        );
    }
    let model = if parsed.model.is_empty() {
        requested_model.to_string()
    } else {
        parsed.model
    };
    Ok(CompletionResponse {
        content: choice.message.content,
        model,
        usage: usage::parse(parsed.usage),
    })
}

/// Why a call did not give an answer.
enum Failure {
    /// The server answered with an error status.
    Http {
        status: reqwest::StatusCode,
        body: String,
        /// The wait the server asked for, if any.
        server_delay: Option<Duration>,
    },
    /// The request did not complete.
    Network(reqwest::Error),
}

impl Failure {
    async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string());
        let server_delay = server_retry_delay(retry_after.as_deref(), &body);
        Failure::Http {
            status,
            body,
            server_delay,
        }
    }

    /// Worth another attempt: a network failure, or a rate limit / server
    /// error whose requested wait, if any, is short enough to honor (a wait
    /// too long, a daily quota say, is an error, not a pause).
    fn is_transient(&self) -> bool {
        match self {
            Failure::Network(_) => true,
            Failure::Http {
                status,
                server_delay,
                ..
            } => {
                is_retryable_status(*status)
                    && server_delay.is_none_or(|d| d <= MAX_SERVER_RETRY_DELAY)
            }
        }
    }

    fn server_delay(&self) -> Option<Duration> {
        match self {
            Failure::Http { server_delay, .. } => *server_delay,
            Failure::Network(_) => None,
        }
    }

    fn warn(&self, attempt: u32, delay: Duration) {
        match self {
            Failure::Http {
                status,
                server_delay,
                ..
            } => tracing::warn!(
                status = %status,
                attempt,
                server_requested = server_delay.is_some(),
                "transient OpenRouter response, retrying in {delay:?}"
            ),
            Failure::Network(source) => tracing::warn!(
                error = %source,
                attempt,
                "OpenRouter network failure, retrying in {delay:?}"
            ),
        }
    }

    fn into_error(self) -> LlmError {
        match self {
            Failure::Http { status, body, .. } => {
                LlmError::InvalidResponse(format!("HTTP status {status}: {body}"))
            }
            Failure::Network(source) => LlmError::Transport(source.to_string()),
        }
    }
}

/// Wait before retry number `attempt` (1-based): exponential backoff, or the
/// server's requested delay when that is longer.
fn retry_delay(attempt: u32, server_delay: Option<Duration>) -> Duration {
    let backoff = RETRY_BASE_DELAY * 2u32.pow(attempt - 1);
    server_delay.map_or(backoff, |d| d.max(backoff))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChatMessage, Role, Usage};

    fn request() -> CompletionRequest {
        CompletionRequest {
            messages: Vec::new(),
            model: None,
        }
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
    fn retry_delay_backs_off_exponentially_unless_the_server_asks_for_more() {
        assert_eq!(retry_delay(1, None), Duration::from_millis(500));
        assert_eq!(retry_delay(3, None), Duration::from_secs(2));
        assert_eq!(
            retry_delay(1, Some(Duration::from_secs(44))),
            Duration::from_secs(44)
        );
        assert_eq!(
            retry_delay(3, Some(Duration::from_millis(100))),
            Duration::from_secs(2)
        );
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
        }
        .with_endpoint(format!("http://{addr}"));
        provider.complete(request()).await.unwrap()
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
                completion_tokens: 35
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
