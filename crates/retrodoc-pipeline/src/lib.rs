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
//! and file role rules ([`roles`]); step 2: models and glossary
//! ([`glossary`]); step 3: entry points and outputs
//! ([`entry_points`]); step 4: the [`surface`] that domains are
//! clustered from.

pub mod actors;
mod artifact;
mod batched_read;
pub mod behaviour_coverage;
pub mod benchmark;
pub mod bm25;
pub mod brief;
pub mod business_files;
pub mod business_read;
pub mod cache;
mod chunk_check;
pub mod chunks;
pub mod confidence;
pub mod contexts;
pub mod diagrams;
pub mod domains;
pub mod entry_points;
pub mod error;
pub mod features;
mod fingerprints;
pub mod glossary;
mod naming;
mod progress;
pub mod ranking;
pub mod repo_map;
pub mod report;
mod response;
pub mod roles;
pub mod rules;
pub mod slices;
pub mod source_hashes;
pub mod sources;
pub mod surface;
#[cfg(test)]
mod testing;
pub mod usage_log;
pub mod use_cases;
pub mod vocabulary;

pub use actors::{authorization_files, build_actors, Actors, BusinessActor};
pub use artifact::{Artifact, Retention};
pub use behaviour_coverage::{
    save_technical_support, save_use_case_coverage, BehaviourCoverage, EntryCount, FeatureCoverage,
};
pub use bm25::Bm25;
pub use brief::{build_brief, sample_chars, Claim, Evidence, ProductBrief};
pub use business_files::{infer_business_files, BusinessEntry, BusinessMap};
pub use business_read::read_business_files;
pub use confidence::{mechanical_score, score_confidence, StepTally};
pub use contexts::{
    build_context_map, contexts_of, domain_vocabulary, domains_from_contexts, family_groups,
    mark_done, mechanical_groups, name_words, plan_run, BoundedContext, ContextMap, EntryOwners,
    Groups, Resolved, ResolvedContext, RunSlice,
};
pub use diagrams::{attach_diagrams, sequence_diagram};
pub use domains::{
    build_domains, build_domains_from_surface, CoverageReport, DomainCluster, DomainMap,
    SubDomainCluster,
};
pub use entry_points::{
    build_entry_points, EntryKind, EntryPoint, EntryPoints, Output, OutputKind,
};
pub use error::PipelineError;
pub use features::{
    build_features, build_features_from_surface, entry_point_id, load_features, save_features,
};
pub use glossary::{build_glossary, Entity, Glossary, MergedEntity};
pub use ranking::{apply_budget, rank_files, Scope};
pub use repo_map::{
    build_repo_map, estimate_repo_map, FileSummary, ModuleSummary, RepoMap, RepoMapEstimate,
    RepoMapOptions,
};
pub use report::{build_report, domain_models, DebtReport};
pub use roles::{identify_roles, FileRole, RoleMap, RoleRule, RoleRules};
pub use rules::{build_rules, load_rules, save_rules, RuleInput};
pub use slices::CodeIndex;
pub use source_hashes::SourceHashes;
pub use sources::{infer_sources, saved_or_sniffed, SourceMapFile};
pub use surface::{Resource, Surface};
pub use use_cases::{build_use_cases, load_use_cases, save_use_cases, UseCaseContext};
pub use vocabulary::{score_business_language, score_unscored_business_language};
