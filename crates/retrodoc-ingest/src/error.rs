use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("repo not found or invalid at {0}")]
    InvalidRepo(PathBuf),
    #[error("error walking the repo: {0}")]
    Walk(#[from] ignore::Error),
    #[error("git error: {0}")]
    Git(#[from] git2::Error),
    #[error("error reading {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid exclusion pattern \"{pattern}\": {source}")]
    InvalidIgnorePattern {
        pattern: String,
        #[source]
        source: ignore::Error,
    },
}
