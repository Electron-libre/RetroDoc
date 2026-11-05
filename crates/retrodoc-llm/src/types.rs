//! The types shared by every provider: messages, requests, responses, the
//! error, and the [`LlmProvider`] contract.

use async_trait::async_trait;

use crate::usage::Usage;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    /// The tools an assistant message asked for, given back in the history
    /// of the next request (empty for every other message).
    pub tool_calls: Vec<ToolCall>,
    /// The call a [`Role::Tool`] message answers.
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// A message holding text only.
    #[must_use]
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// An assistant answer that asked for tools, to give back in the history:
    /// its text (often empty) and its calls.
    #[must_use]
    pub fn assistant_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            tool_calls,
            ..Self::new(Role::Assistant, content)
        }
    }

    /// The result of the call `call_id`.
    #[must_use]
    pub fn tool_result(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: Some(call_id.into()),
            ..Self::new(Role::Tool, content)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    /// The result of a tool call (`specs/tool-calling.md`).
    Tool,
}

impl Role {
    pub(crate) fn as_api_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompletionRequest {
    pub messages: Vec<ChatMessage>,
    /// Model to use; overrides the one from the config if present.
    pub model: Option<String>,
    /// Shape of the JSON answer the caller expects. A provider that supports
    /// it asks the server to constrain the answer to it; the caller still
    /// parses leniently, as the schema may be ignored or refused.
    pub json_schema: Option<ResponseSchema>,
    /// The tools the model may call instead of answering in text (none: no
    /// `tools` sent). ADR 0046, `specs/tool-calling.md`.
    pub tools: Vec<ToolSpec>,
}

/// A tool offered to the model.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON schema of the arguments (an `object`).
    pub parameters: serde_json::Value,
}

/// A call to a tool the model asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// The id the server gave the call; its result names it back.
    pub id: String,
    pub name: String,
    /// The arguments as the raw text the server sent (meant to be a JSON
    /// object, never checked: the caller parses them).
    pub arguments: String,
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
    /// The server cut the answer at its completion limit (`finish_reason=length`): `content` is
    /// incomplete, and asking the same question again would be cut at the same place.
    pub truncated: bool,
    /// The tools the model asked for, in the order of the answer (empty for
    /// a text answer).
    pub tool_calls: Vec<ToolCall>,
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

    /// The model this provider asks for when a request names none. The
    /// passes put it in the fingerprint of their cached results, so a new
    /// model redoes them (ADR 0023). Empty when the provider has no fixed one.
    #[allow(clippy::unnecessary_literal_bound)] // implementations return a borrowed field
    fn model(&self) -> &str {
        ""
    }

    /// The language this provider asks the answers in, when it asks one (ADR 0041). The passes whose
    /// print holds no model put it in their print instead.
    fn language(&self) -> Option<&str> {
        None
    }
}
