//! The `OpenRouter` provider (OpenAI-compatible chat completions): the HTTP
//! call, its retries and the decoding of the answer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use retrodoc_core::config::LlmConfig;
use serde::{Deserialize, Serialize};

use crate::usage;
use crate::{CompletionRequest, CompletionResponse, LlmError, LlmProvider, ResponseSchema};

const OPENROUTER_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
/// Endpoint of the `DeepSeek` API, which speaks the same chat-completions format.
const DEEPSEEK_ENDPOINT: &str = "https://api.deepseek.com/chat/completions";
/// API key variable of `openrouter`, the default of `llm.api_key_env`.
const OPENROUTER_KEY_ENV: &str = "OPENROUTER_API_KEY";
const DEEPSEEK_KEY_ENV: &str = "DEEPSEEK_API_KEY";
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
const MAX_COMPLETION_TOKENS: u32 = 8_192;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ApiResponseFormat<'a>>,
    /// `OpenRouter` routing preferences, sent with a schema only.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<ApiProviderPreferences>,
}

#[derive(Debug, Serialize)]
struct ApiResponseFormat<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    json_schema: ApiJsonSchema<'a>,
}

#[derive(Debug, Serialize)]
struct ApiJsonSchema<'a> {
    name: &'a str,
    strict: bool,
    schema: &'a serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ApiProviderPreferences {
    require_parameters: bool,
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
    /// `llm.structured_output`: send the schema of a request that has one.
    structured_output: bool,
    /// Ask `OpenRouter` to route only to servers that honor every parameter
    /// (the real endpoint, not a `base_url` override).
    require_parameters: bool,
    /// Set once a server refused the schema and accepted the plain request:
    /// the schema is not sent again.
    schema_refused: AtomicBool,
}

impl OpenRouterProvider {
    /// Builds the provider from the config, reading the API key from the
    /// environment variable named by `llm.api_key_env` (for `deepseek`, left
    /// at its `OpenRouter` default, `DEEPSEEK_API_KEY`). Calls the endpoint of
    /// `llm.provider` (`openrouter` or `deepseek`, the same wire format)
    /// unless `llm.base_url` overrides it (e.g. to point at a local
    /// OpenAI-compatible server instead).
    ///
    /// # Errors
    ///
    /// Returns an error if `config.provider` isn't `"openrouter"` or `"deepseek"`, if the
    /// API key's environment variable is not set, or if the underlying
    /// HTTP client can't be built.
    pub fn from_config(config: &LlmConfig) -> Result<Self, LlmError> {
        let (default_endpoint, key_env) = match config.provider.as_str() {
            "openrouter" => (OPENROUTER_ENDPOINT, config.api_key_env.as_str()),
            "deepseek" if config.api_key_env == OPENROUTER_KEY_ENV => {
                (DEEPSEEK_ENDPOINT, DEEPSEEK_KEY_ENV)
            }
            "deepseek" => (DEEPSEEK_ENDPOINT, config.api_key_env.as_str()),
            other => return Err(LlmError::UnsupportedProvider(other.to_string())),
        };
        let api_key =
            std::env::var(key_env).map_err(|_| LlmError::MissingApiKey(key_env.to_string()))?;
        let endpoint = config
            .base_url
            .clone()
            .unwrap_or_else(|| default_endpoint.to_string());
        let require_parameters = config.provider == "openrouter" && config.base_url.is_none();
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
            structured_output: config.structured_output.unwrap_or(true),
            require_parameters,
            schema_refused: AtomicBool::new(false),
        })
    }

    #[cfg(test)]
    fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }
}

impl OpenRouterProvider {
    /// The schema to send with this request, unless the setting is off or a
    /// server already refused it.
    fn schema_to_send<'a>(&self, request: &'a CompletionRequest) -> Option<&'a ResponseSchema> {
        if !self.structured_output || self.schema_refused.load(Ordering::Relaxed) {
            return None;
        }
        request.json_schema.as_ref()
    }

    /// Sends the request (with the retries on transient errors) and decodes
    /// the answer.
    async fn send(
        &self,
        request: &CompletionRequest,
        schema: Option<&ResponseSchema>,
    ) -> Result<CompletionResponse, Failure> {
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
            response_format: schema.map(|s| ApiResponseFormat {
                kind: "json_schema",
                json_schema: ApiJsonSchema {
                    name: &s.name,
                    strict: true,
                    schema: &s.schema,
                },
            }),
            provider: (schema.is_some() && self.require_parameters).then_some(
                ApiProviderPreferences {
                    require_parameters: true,
                },
            ),
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
                    return decode_success(response, model)
                        .await
                        .map_err(Failure::Decode);
                }
                Ok(response) => Failure::from_response(response).await,
                Err(source) => Failure::Network(source),
            };

            if attempt >= MAX_RETRIES || !failure.is_transient() {
                return Err(failure);
            }
            attempt += 1;
            let delay = retry_delay(attempt, failure.server_delay());
            failure.warn(attempt, delay);
            tokio::time::sleep(delay).await;
        }
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
        let Some(schema) = self.schema_to_send(&request) else {
            return self.send(&request, None).await.map_err(Failure::into_error);
        };
        match self.send(&request, Some(schema)).await {
            Ok(response) => Ok(response),
            Err(failure) if failure.may_be_a_refused_schema() => {
                // The server may not know `response_format` (or this model
                // not support it). Try again plain; only if that works is the
                // schema the culprit and left out for the rest of the run.
                let response = self
                    .send(&request, None)
                    .await
                    .map_err(|_| failure.into_error())?;
                if !self.schema_refused.swap(true, Ordering::Relaxed) {
                    tracing::warn!(
                        "the server refused the JSON schema (response_format); \
                         continuing without it (set llm.structured_output = false to silence)"
                    );
                }
                Ok(response)
            }
            Err(failure) => Err(failure.into_error()),
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
    /// The server answered successfully but the body is unusable.
    Decode(LlmError),
}

impl Failure {
    /// A client error that a schema the server cannot handle would explain:
    /// bad request, not found (no endpoint with the parameter), unprocessable.
    fn may_be_a_refused_schema(&self) -> bool {
        matches!(
            self,
            Failure::Http { status, .. }
                if [400, 404, 422].contains(&status.as_u16())
        )
    }

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
            Failure::Decode(_) => false,
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
            Failure::Network(_) | Failure::Decode(_) => None,
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
            Failure::Decode(_) => {}
        }
    }

    fn into_error(self) -> LlmError {
        match self {
            Failure::Http { status, body, .. } => {
                LlmError::InvalidResponse(format!("HTTP status {status}: {body}"))
            }
            Failure::Network(source) => LlmError::Transport(source.to_string()),
            Failure::Decode(err) => err,
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
            json_schema: None,
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
            structured_output: None,
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
            structured_output: None,
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
            structured_output: None,
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
    fn deepseek_uses_its_endpoint_and_key_variable() {
        let config = LlmConfig {
            provider: "deepseek".to_string(),
            model: "deepseek-chat".to_string(),
            ..LlmConfig::default()
        };
        std::env::remove_var(DEEPSEEK_KEY_ENV);
        let missing = OpenRouterProvider::from_config(&config);
        assert!(matches!(missing, Err(LlmError::MissingApiKey(v)) if v == DEEPSEEK_KEY_ENV));
        std::env::set_var(DEEPSEEK_KEY_ENV, "unused");
        let provider = OpenRouterProvider::from_config(&config).unwrap();
        assert_eq!(provider.endpoint, DEEPSEEK_ENDPOINT);
        std::env::remove_var(DEEPSEEK_KEY_ENV);
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
            structured_output: None,
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
            structured_output: true,
            require_parameters: false,
            schema_refused: AtomicBool::new(false),
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
            structured_output: true,
            require_parameters: false,
            schema_refused: AtomicBool::new(false),
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
            structured_output: true,
            require_parameters: false,
            schema_refused: AtomicBool::new(false),
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
            structured_output: true,
            require_parameters: false,
            schema_refused: AtomicBool::new(false),
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
                structured_output: true,
                require_parameters: false,
                schema_refused: AtomicBool::new(false),
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
            structured_output,
            require_parameters: true,
            schema_refused: AtomicBool::new(false),
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

    #[test]
    fn timeout_defaults_to_two_minutes_and_follows_the_config() {
        let mut config = LlmConfig::default();
        assert_eq!(request_timeout(&config), Duration::from_secs(120));
        config.timeout_secs = Some(600);
        assert_eq!(request_timeout(&config), Duration::from_secs(600));
    }
}
