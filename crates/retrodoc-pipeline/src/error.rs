use std::path::PathBuf;

use retrodoc_llm::LlmError;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("LLM call failed: {0}")]
    Llm(#[from] LlmError),
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse an LLM response as JSON: {source}\n--- raw response ---\n{raw}")]
    ResponseParse {
        raw: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not serialize an artifact to JSON: {0}")]
    JsonSerialize(#[from] serde_json::Error),
    #[error("could not serialize an artifact to YAML: {0}")]
    YamlSerialize(#[from] serde_yaml::Error),
    #[error("benchmark reference {path} is unusable: {reason}")]
    InvalidReference { path: PathBuf, reason: String },
    #[error("artifact unreadable/unwritable at {path}: {source}")]
    ArtifactIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
