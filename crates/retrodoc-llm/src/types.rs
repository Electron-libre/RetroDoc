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
    #[error("LLM provider \"{0}\" is not supported in v1 (only openrouter is)")]
    UnsupportedProvider(String),
}

/// Contract shared by every LLM provider, so other providers than
/// `OpenRouter` can be added without touching the rest of the pipeline.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;
}
