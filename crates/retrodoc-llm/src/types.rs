//! The types shared by every provider: messages, requests, responses, the
//! error, and the [`LlmProvider`] contract.

use async_trait::async_trait;

use crate::usage::Usage;

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
    pub(crate) fn as_api_str(self) -> &'static str {
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
    /// Shape of the JSON answer the caller expects. A provider that supports
    /// it asks the server to constrain the answer to it; the caller still
    /// parses leniently, as the schema may be ignored or refused.
    pub json_schema: Option<ResponseSchema>,
}

/// A JSON schema the answer should follow (sent as `response_format`).
#[derive(Debug, Clone)]
pub struct ResponseSchema {
    /// Name of the schema (letters, digits, `_` and `-`).
    pub name: String,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Default)]
pub struct CompletionResponse {
    pub content: String,
    pub model: String,
    /// Tokens the server billed for this call, when it says so. `None` when
    /// the server sends no (or an incomplete) `usage` block: never estimated.
    pub usage: Option<Usage>,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("environment variable {0} is not set (LLM API key)")]
    MissingApiKey(String),
    #[error("network call to the LLM provider failed: {0}")]
    Transport(String),
    #[error("invalid response from the LLM provider: {0}")]
    InvalidResponse(String),
    #[error("LLM provider \"{0}\" is not supported (openrouter and deepseek are)")]
    UnsupportedProvider(String),
}

/// Contract shared by every LLM provider, so other providers than
/// `OpenRouter` can be added without touching the rest of the pipeline.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;

    /// Tells the provider that the answer it just gave could not be parsed
    /// by the caller (`skipped`: the caller gave up on that unit instead of
    /// asking again). Providers that count things override it; the wrappers
    /// pass it on.
    fn note_unparseable_answer(&self, _skipped: bool) {}
}
