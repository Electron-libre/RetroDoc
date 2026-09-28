//! `retrodoc-pipeline`: orchestration of the generation pipeline passes
//! (PLAN.md §2) and incremental cache (`.retrodoc/cache/`).
//!
//! Roadmap phase 2 (PLAN.md §5): repo map — bottom-up summaries per
//! file/module, enriched with git history ([`repo_map`]). Roadmap phase 3:
//! domain/sub-domain clustering of the repo map, with coverage validation
//! ([`domains`]). The features → use cases → diagrams → confidence passes
//! arrive in later roadmap phases.

pub mod cache;
pub mod domains;
pub mod error;
pub mod repo_map;

pub use domains::{build_domains, CoverageReport, DomainCluster, DomainMap, SubDomainCluster};
pub use error::PipelineError;
pub use repo_map::{build_repo_map, FileSummary, ModuleSummary, RepoMap};
