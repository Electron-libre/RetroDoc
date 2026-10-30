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
/// (429 / 5xx) — docs/ARCHITECTURE.md §8 "retry, rate-limit".
const MAX_RETRIES: u32 = 3;
const RETRY_BASE_DELAY: Duration = Duration::from_millis(500);
/// Longest wait the provider may ask for (`Retry-After`, `retryDelay`) that
/// is still honored; beyond it the error is returned (e.g. a daily quota).
const MAX_SERVER_RETRY_DELAY: Duration = Duration::from_secs(120);
/// Default per-request HTTP timeout of a hosted or non-loopback endpoint
/// (`llm.timeout_secs` overrides it). `OpenRouter` always responds well within
/// this, and a model can degenerate into a runaway generation loop that never
/// reaches a stop token; without a client-side bound the call hangs
/// indefinitely instead of surfacing an error.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
/// Default timeout when `llm.base_url` names a loopback host: a local server
/// writes a long structured answer far slower than a hosted one (the
/// [`MAX_COMPLETION_TOKENS`] cap at about 15 tokens per second is some 550 s),
/// while a hosted provider that hangs must still fail within
/// [`DEFAULT_REQUEST_TIMEOUT`].
const LOCAL_REQUEST_TIMEOUT: Duration = Duration::from_mins(15);
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
    temperature: Option<f32>,
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
    temperature: Option<f32>,
    /// The per-request timeout of `client`, named in the error of a call that runs past it.
    timeout: Duration,
    /// `llm.structured_output`: send the schema of a request that has one.
    structured_output: bool,
    /// Ask `OpenRouter` to route only to servers that honor every parameter
    /// (the real endpoint, not a `base_url` override).
    require_parameters: bool,
    /// Set once a server refused the schema and accepted the plain request:
    /// the schema is not sent again.
    schema_refused: AtomicBool,
    /// Set once a server refused the temperature and accepted the request
    /// without it: the temperature is not sent again.
    temperature_refused: AtomicBool,
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
        let timeout = request_timeout(config);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|source| LlmError::Transport(source.to_string()))?;
        Ok(Self {
            client,
            api_key,
            default_model: config.model.clone(),
            endpoint,
            reasoning_effort: config.reasoning_effort.clone(),
            temperature: config.temperature,
            structured_output: config.structured_output.unwrap_or(true),
            require_parameters,
            schema_refused: AtomicBool::new(false),
            temperature_refused: AtomicBool::new(false),
            timeout,
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

    /// The temperature to send, unless unset or a server already refused it.
    fn temperature_to_send(&self) -> Option<f32> {
        if self.temperature_refused.load(Ordering::Relaxed) {
            return None;
        }
        self.temperature
    }

    /// Sends the request (with the retries on transient errors) and decodes
    /// the answer.
    async fn send(
        &self,
        request: &CompletionRequest,
        schema: Option<&ResponseSchema>,
        temperature: Option<f32>,
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
            temperature,
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

/// Per-request timeout: `llm.timeout_secs`, else [`LOCAL_REQUEST_TIMEOUT`]
/// for a loopback `llm.base_url`, else [`DEFAULT_REQUEST_TIMEOUT`].
fn request_timeout(config: &LlmConfig) -> Duration {
    if let Some(secs) = config.timeout_secs {
        return Duration::from_secs(secs);
    }
    if config.base_url.as_deref().is_some_and(is_loopback_url) {
        LOCAL_REQUEST_TIMEOUT
    } else {
        DEFAULT_REQUEST_TIMEOUT
    }
}

/// The host of `url` is `localhost`, an address of `127.0.0.0/8` or `::1`.
fn is_loopback_url(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    let host = url.host_str().unwrap_or_default();
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
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
        let schema = self.schema_to_send(&request);
        let temperature = self.temperature_to_send();
        let failure = match self.send(&request, schema, temperature).await {
            Ok(response) => return Ok(response),
            Err(failure) => failure,
        };
        // A server may not know `temperature` or `response_format` (or this
        // model not take them). Try again without one, then the other, then
        // both; only if a request works are the parameters left out of it the
        // culprits, and they are not sent again for the rest of the run.
        let drop_temperature = [true, false, true];
        let drop_schema = [false, true, true];
        for (drop_temperature, drop_schema) in drop_temperature.into_iter().zip(drop_schema) {
            if (drop_temperature && temperature.is_none())
                || (drop_schema && schema.is_none())
                || !failure.may_be_a_refused_parameter()
            {
                continue;
            }
            let attempt = self
                .send(
                    &request,
                    schema.filter(|_| !drop_schema),
                    temperature.filter(|_| !drop_temperature),
                )
                .await;
            let Ok(response) = attempt else { continue };
            if drop_temperature && !self.temperature_refused.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    "the server refused the temperature; continuing without it \
                     (unset llm.temperature to silence)"
                );
            }
            if drop_schema && !self.schema_refused.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    "the server refused the JSON schema (response_format); \
                     continuing without it (set llm.structured_output = false to silence)"
                );
            }
            return Ok(response);
        }
        Err(failure.into_error(self.timeout))
    }

    fn model(&self) -> &str {
        &self.default_model
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
    let truncated = choice.finish_reason.as_deref() == Some("length");
    if truncated {
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
        truncated,
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

/// A timeout of the server's answer, as opposed to one while connecting.
trait WaitingTooLong {
    fn is_waiting_too_long(&self) -> bool;
}

impl WaitingTooLong for reqwest::Error {
    fn is_waiting_too_long(&self) -> bool {
        self.is_timeout() && !self.is_connect()
    }
}

impl Failure {
    /// A client error that a parameter the server cannot handle (the schema,
    /// the temperature) would explain:
    /// bad request, not found (no endpoint with the parameter), unprocessable.
    fn may_be_a_refused_parameter(&self) -> bool {
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
            // A timeout is not: the same prompt takes as long again, so a retry only multiplies the wait.
            Failure::Network(source) => !source.is_waiting_too_long(),
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

    /// The error of the last attempt; `timeout` is the one of the client, named when it was reached.
    fn into_error(self, timeout: Duration) -> LlmError {
        match self {
            Failure::Http { status, body, .. } => {
                LlmError::InvalidResponse(format!("HTTP status {status}: {body}"))
            }
            Failure::Network(source) if source.is_waiting_too_long() => LlmError::Transport(format!(
                "the request timed out after {timeout:?} before the model finished its answer (raise `llm.timeout_secs`): {source}"
            )),
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
mod tests;
