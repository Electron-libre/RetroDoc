use retrodoc_core::config::LlmConfig;

use super::super::{OpenRouterProvider, DEEPSEEK_ENDPOINT, DEEPSEEK_KEY_ENV, OPENROUTER_ENDPOINT};
use crate::LlmError;

#[test]
fn missing_api_key_env_is_reported() {
    let config = LlmConfig {
        provider: "openrouter".to_string(),
        api_key_env: "RETRODOC_TEST_MISSING_KEY_VAR".to_string(),
        model: "anthropic/claude-sonnet-4.5".to_string(),
        base_url: None,
        reasoning_effort: None,
        temperature: None,
        timeout_secs: None,
        concurrency: None,
        structured_output: None,
        passes: std::collections::BTreeMap::new(),
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
        temperature: None,
        timeout_secs: None,
        concurrency: None,
        structured_output: None,
        passes: std::collections::BTreeMap::new(),
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
        temperature: None,
        timeout_secs: None,
        concurrency: None,
        structured_output: None,
        passes: std::collections::BTreeMap::new(),
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
        temperature: None,
        timeout_secs: None,
        concurrency: None,
        structured_output: None,
        passes: std::collections::BTreeMap::new(),
    };
    std::env::set_var(&config.api_key_env, "unused");
    let provider = OpenRouterProvider::from_config(&config).unwrap();
    assert_eq!(provider.endpoint, OPENROUTER_ENDPOINT);
    std::env::remove_var(&config.api_key_env);
}
