//! `retrodoc-pipeline`: orchestration of the generation pipeline passes
//! (PLAN.md §2) and incremental cache (`.retrodoc/cache/`).
//!
//! Roadmap phase 2 (PLAN.md §5): repo map — bottom-up summaries per
//! file/module, enriched with git history ([`repo_map`]). Roadmap phase 3:
//! domain/sub-domain clustering of the repo map, with coverage validation
//! ([`domains`]). Roadmap phase 4: features ([`features`]) → use cases
//! ([`use_cases`]) → Mermaid diagrams ([`diagrams`]). Roadmap phase 5:
//! confidence scoring ([`confidence`]) and the documentation debt report
//! ([`report`]). Roadmap phase 7 (PLAN.md §7.1), step 1: stack identification
//! and file role rules ([`roles`]).

pub mod cache;
pub mod confidence;
pub mod diagrams;
pub mod domains;
pub mod error;
pub mod features;
mod fingerprints;
pub mod repo_map;
pub mod report;
mod response;
pub mod roles;
pub mod use_cases;

pub use confidence::score_confidence;
pub use diagrams::{attach_diagrams, sequence_diagram};
pub use domains::{build_domains, CoverageReport, DomainCluster, DomainMap, SubDomainCluster};
pub use error::PipelineError;
pub use features::{build_features, load_features, save_features};
pub use repo_map::{build_repo_map, FileSummary, ModuleSummary, RepoMap};
pub use report::{build_report, domain_models, DebtReport};
pub use roles::{identify_roles, FileRole, RoleMap, RoleRule, RoleRules};
pub use use_cases::{build_use_cases, load_use_cases, save_use_cases};
