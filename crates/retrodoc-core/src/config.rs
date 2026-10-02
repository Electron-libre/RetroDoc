//! `retrodoc.toml` configuration.
//!
//! The file lives at the root of the analyzed repo and describes the LLM
//! provider (`OpenRouter` in v1) as well as the ingestion settings (paths
//! ignored in addition to `.gitignore`, doc output folder).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const CONFIG_FILE_NAME: &str = "retrodoc.toml";

/// Default model proposed at init. Can be changed in `retrodoc.toml`.
pub const DEFAULT_MODEL: &str = "anthropic/claude-sonnet-4.5";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub ingest: IngestConfig,
    #[serde(default)]
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// LLM provider. Only "openrouter" is supported in v1.
    #[serde(default = "LlmConfig::default_provider")]
    pub provider: String,
    /// Name of the environment variable holding the API key (never the key
    /// itself: this file is versioned alongside the analyzed repo).
    #[serde(default = "LlmConfig::default_api_key_env")]
    pub api_key_env: String,
    /// `OpenRouter` model to use (e.g. "anthropic/claude-sonnet-4.5").
    #[serde(default = "LlmConfig::default_model")]
    pub model: String,
    /// Overrides the chat-completions endpoint `OpenRouterProvider` calls.
    /// Unset: `OpenRouter`'s own endpoint. Set: any server speaking the
    /// same OpenAI-compatible chat-completions wire format — e.g. a local
    /// Ollama/LM Studio/llama.cpp instance — since `OpenRouterProvider`'s
    /// request/response shapes are that same protocol, not `OpenRouter`-
    /// specific. Still not "multi-provider support" (out of scope for v1,
    /// PLAN.md §1): the auth model and client stay `OpenRouterProvider`'s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Sent as `reasoning_effort` with every request when set (e.g.
    /// `"none"`, `"low"`). Lets a "thinking" model (Qwen3, …) skip its
    /// internal reasoning, which is far too slow for `RetroDoc`'s many short
    /// structured calls. Unset: nothing is sent, the server's default applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// Per-request HTTP timeout in seconds. Unset: 120. A slow local model
    /// producing a long structured answer (several thousand tokens at a few
    /// tokens per second) needs more; the retry loop restarts the whole
    /// generation on a timeout, so too small a value never succeeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Maximum number of LLM calls in flight at once for the passes that
    /// can run them in parallel (file and directory summaries). Unset: 1.
    /// A single local model gains little; a hosted one (`OpenRouter`) a lot,
    /// within its rate limits (429s are retried with backoff).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<usize>,
}

impl LlmConfig {
    fn default_provider() -> String {
        "openrouter".to_string()
    }
    fn default_api_key_env() -> String {
        "OPENROUTER_API_KEY".to_string()
    }
    fn default_model() -> String {
        DEFAULT_MODEL.to_string()
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: Self::default_provider(),
            api_key_env: Self::default_api_key_env(),
            model: Self::default_model(),
            base_url: None,
            reasoning_effort: None,
            timeout_secs: None,
            concurrency: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestConfig {
    /// Extra patterns to ignore, in addition to `.gitignore` (gitignore syntax).
    #[serde(default)]
    pub extra_ignore: Vec<String>,
    /// Existing Markdown documentation folders to take as input.
    #[serde(default = "IngestConfig::default_existing_docs_paths")]
    pub existing_docs_paths: Vec<String>,
}

impl IngestConfig {
    fn default_existing_docs_paths() -> Vec<String> {
        vec!["docs".to_string(), "README.md".to_string()]
    }
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            extra_ignore: Vec::new(),
            existing_docs_paths: Self::default_existing_docs_paths(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output folder for the generated docs, relative to the repo root.
    #[serde(default = "OutputConfig::default_docs_dir")]
    pub docs_dir: String,
}

impl OutputConfig {
    fn default_docs_dir() -> String {
        "docs".to_string()
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            docs_dir: Self::default_docs_dir(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(String),
    #[error("error reading {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("error writing {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid TOML config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("could not serialize TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
}

impl Config {
    /// Loads the config from `<repo_root>/retrodoc.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file is missing, unreadable, or its TOML
    /// content is invalid.
    pub fn load(repo_root: &Path) -> Result<Self, ConfigError> {
        let path = repo_root.join(CONFIG_FILE_NAME);
        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }
        let raw = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let config: Config = toml::from_str(&raw)?;
        Ok(config)
    }

    /// Writes the default config to `<repo_root>/retrodoc.toml`.
    /// Fails if the file already exists (see `force` on the CLI side to overwrite).
    ///
    /// # Errors
    ///
    /// Returns an error if the file already exists without `force`, or if
    /// writing to disk fails.
    pub fn write_default(repo_root: &Path, force: bool) -> Result<Self, ConfigError> {
        let path = repo_root.join(CONFIG_FILE_NAME);
        if path.exists() && !force {
            return Err(ConfigError::Write {
                path: path.display().to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "retrodoc.toml already exists (use --force to overwrite)",
                ),
            });
        }
        let config = Config::default();
        let raw = toml::to_string_pretty(&config)?;
        fs::write(&path, raw).map_err(|source| ConfigError::Write {
            path: path.display().to_string(),
            source,
        })?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrips_through_toml() {
        let config = Config::default();
        let raw = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&raw).unwrap();
        assert_eq!(parsed.llm.provider, config.llm.provider);
        assert_eq!(parsed.llm.model, config.llm.model);
        assert_eq!(parsed.output.docs_dir, config.output.docs_dir);
    }

    #[test]
    fn partial_toml_falls_back_to_defaults() {
        let raw = r#"
            [llm]
            model = "openai/gpt-4o"
        "#;
        let parsed: Config = toml::from_str(raw).unwrap();
        assert_eq!(parsed.llm.model, "openai/gpt-4o");
        assert_eq!(parsed.llm.provider, "openrouter");
        assert_eq!(parsed.output.docs_dir, "docs");
    }
}
