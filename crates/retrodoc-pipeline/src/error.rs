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
    #[error("domains.yaml unreadable/unwritable at {path}: {source}")]
    DomainsIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize domains.yaml: {0}")]
    DomainsSerialize(#[from] serde_yaml::Error),
    #[error("could not parse an LLM response as JSON: {source}\n--- raw response ---\n{raw}")]
    ResponseParse {
        raw: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("intermediate artifact unreadable/unwritable at {path}: {source}")]
    ArtifactIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
