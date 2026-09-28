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
    #[error("repo map cache unreadable/unwritable at {path}: {source}")]
    Cache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize the repo map cache: {0}")]
    CacheSerialize(#[from] serde_json::Error),
}
