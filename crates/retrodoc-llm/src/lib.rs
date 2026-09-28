//! `retrodoc-llm`: LLM provider abstraction (`LlmProvider`), so RetroDoc
//! isn't locked to OpenRouter (PLAN.md §1). Only OpenRouter is implemented
//! in v1; the network call itself arrives at the "repo map" phase (first
//! pipeline step to consume an LLM) — for now this crate only defines the
//! contract and the configuration.

use async_trait::async_trait;
use retrodoc_core::config::LlmConfig;

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
/// OpenRouter can be added without touching the rest of the pipeline.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;
}

/// OpenRouter provider (https://openrouter.ai). The actual HTTP call is
/// wired up at the phase where the pipeline first needs it.
pub struct OpenRouterProvider {
    api_key: String,
    default_model: String,
}

impl OpenRouterProvider {
    /// Builds the provider from the config, reading the API key from the
    /// environment variable named by `llm.api_key_env`.
    pub fn from_config(config: &LlmConfig) -> Result<Self, LlmError> {
        if config.provider != "openrouter" {
            return Err(LlmError::UnsupportedProvider(config.provider.clone()));
        }
        let api_key = std::env::var(&config.api_key_env)
            .map_err(|_| LlmError::MissingApiKey(config.api_key_env.clone()))?;
        Ok(Self {
            api_key,
            default_model: config.model.clone(),
        })
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        // TODO(repo map phase): real HTTP call to https://openrouter.ai/api/v1/chat/completions,
        // with retry + rate-limit handling (PLAN.md, retrodoc-llm crate).
        let _ = (&self.api_key, &self.default_model);
        Err(LlmError::Transport(
            "OpenRouterProvider::complete is not implemented yet (repo map phase)".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_api_key_env_is_reported() {
        let config = LlmConfig {
            provider: "openrouter".to_string(),
            api_key_env: "RETRODOC_TEST_MISSING_KEY_VAR".to_string(),
            model: "anthropic/claude-sonnet-4.5".to_string(),
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
        };
        let result = OpenRouterProvider::from_config(&config);
        assert!(matches!(result, Err(LlmError::UnsupportedProvider(p)) if p == "openai"));
    }
}
