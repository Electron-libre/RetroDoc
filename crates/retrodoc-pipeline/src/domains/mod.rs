//! Domain/sub-domain clustering (PLAN.md §2 step 3, roadmap phase 3): the
//! LLM groups the repo map (+ existing docs) into a business-oriented
//! domain/sub-domain breakdown, saved as the intermediate `domains.yaml`
//! artifact (PLAN.md §2: "not a final file... the basis for the next
//! pipeline pass"). Distinct from `retrodoc_core::model::Domain`, which is
//! the final rendering model populated once features/use cases are attached
//! in later roadmap phases.
//!
//! The LLM is given `repo_map.modules` (one line per directory), not a flat
//! per-file listing: a flat listing doesn't scale (tens of thousands of
//! tokens on a repo with thousands of files). [`expand_to_files`] then
//! mechanically resolves each file to its most-specific assigned ancestor
//! directory (no extra LLM call) before coverage repairs run.
//!
//! Coverage is enforced by construction rather than by failing the whole
//! run, mirroring `repo_map`'s "skip an unreadable file rather than abort"
//! resilience: an LLM clustering is asked for, not guaranteed, so
//! [`enforce_coverage`] repairs a slightly imperfect response instead of
//! erroring out of `generate` entirely.
//! - A source file the clustering left unassigned is first placed by a second,
//!   small LLM call (its summary and the domains found; see `repair`): a
//!   directory the model skipped or answered with single files would
//!   otherwise leave whole folders undocumented. What that call can't place
//!   is bucketed into a synthetic "uncategorized" domain (100% coverage,
//!   PLAN.md §2 step 3).
//! - A file assigned to more than one domain/sub-domain keeps only its
//!   first assignment (no overlap, PLAN.md §2 step 3).
//! - A path the LLM cited that doesn't match any known file (hallucinated)
//!   is dropped entirely.
//!
//! All three repairs are logged as warnings and returned in a
//! [`CoverageReport`] so the caller can surface them to the user.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::ExistingDoc;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, Fingerprints};
use crate::repo_map::{FileSummary, RepoMap};
use crate::response::{complete_text, parse_json_response};
use crate::surface::Surface;

mod coverage;
mod prompt;
mod repair;
#[cfg(test)]
mod tests;

use self::coverage::{enforce_coverage, expand_to_files};
use self::prompt::{
    clustering_fingerprint, clustering_prompt, layer_named_domains, BUSINESS_NAMING_ADDENDUM,
    DOMAIN_CLUSTERING_SYSTEM_PROMPT,
};
use self::repair::{assign_unplaced_files, unassigned_files};

pub(crate) const UNCATEGORIZED_SLUG: &str = "uncategorized";

/// Intermediate clustering artifact (PLAN.md §2 step 3): a business-oriented
/// domain/sub-domain breakdown of the repo, with every known source file
/// assigned to exactly one domain (and optionally one of its sub-domains).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DomainMap {
    pub domains: Vec<DomainCluster>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainCluster {
    pub slug: String,
    pub name: String,
    pub description: String,
    /// Source files assigned directly to this domain (not to one of its
    /// `sub_domains`).
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub sub_domains: Vec<SubDomainCluster>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubDomainCluster {
    pub slug: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub paths: Vec<PathBuf>,
}

/// What [`enforce_coverage`] found (and repaired) in a clustering response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoverageReport {
    /// Known source files the LLM didn't assign anywhere; bucketed into the
    /// synthetic "uncategorized" domain.
    pub uncovered: Vec<PathBuf>,
    /// Known source files assigned to more than one domain/sub-domain; only
    /// the first assignment (domain order, then sub-domain order) was kept.
    pub overlapping: Vec<PathBuf>,
    /// Paths the LLM cited that don't match any file in the repo map
    /// (hallucinated); dropped entirely.
    pub unknown: Vec<PathBuf>,
}

impl CoverageReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.uncovered.is_empty() && self.overlapping.is_empty() && self.unknown.is_empty()
    }
}

impl DomainMap {
    /// Persists the clustering as `.retrodoc/cache/domains.yaml` (PLAN.md
    /// §2 step 3): a human-reviewable intermediate artifact, the basis for
    /// the next pipeline pass (features, PLAN.md §2 step 4).
    ///
    /// # Errors
    ///
    /// Returns an error if `.retrodoc/cache/` can't be created, the file
    /// can't be written, or serialization to YAML fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::Domains.path(repo_root), self)?;
        Ok(())
    }

    /// Loads a previously saved `domains.yaml`. Missing or unreadable:
    /// `None` (first run), not an error.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Domains.path(repo_root))
    }
}

/// Clusters the repo map into functional domains/sub-domains, repairs the
/// result to guarantee full coverage and no overlap (see module docs), and
/// persists it to `.retrodoc/cache/domains.yaml`.
///
/// # Errors
///
/// Returns an error if the LLM call fails, its response isn't valid JSON
/// matching the expected schema, or the artifact can't be saved to disk.
pub async fn build_domains(
    repo_root: &Path,
    repo_map: &RepoMap,
    existing_docs: &[ExistingDoc],
    surface: &Surface,
    llm: &dyn LlmProvider,
) -> Result<(DomainMap, CoverageReport), PipelineError> {
    let all_paths: Vec<PathBuf> = repo_map.files.iter().map(|f| f.path.clone()).collect();
    if all_paths.is_empty() {
        let map = DomainMap::default();
        map.save(repo_root)?;
        return Ok((map, CoverageReport::default()));
    }

    // The clustering is not deterministic: re-asking for it on identical input
    // yields differently named domains, which invalidates every downstream
    // fingerprint (keyed by domain slug). Keep the saved one while the input
    // (files, file summaries, existing docs) is unchanged.
    let mut prints = Fingerprints::load(repo_root);
    let input_print = clustering_fingerprint(repo_map, existing_docs, surface);
    if prints.domains.as_ref() == Some(&input_print) {
        if let Some(saved) = DomainMap::load(repo_root).filter(|m| !m.domains.is_empty()) {
            tracing::info!("domains unchanged, reused");
            return Ok((saved, CoverageReport::default()));
        }
    }

    let prompt = clustering_prompt(repo_map, existing_docs, surface);
    let system_prompt = if surface.is_empty() {
        DOMAIN_CLUSTERING_SYSTEM_PROMPT.to_string()
    } else {
        format!("{DOMAIN_CLUSTERING_SYSTEM_PROMPT}{BUSINESS_NAMING_ADDENDUM}")
    };
    let response = complete_text(llm, &system_prompt, &prompt).await?;

    let map: DomainMap = parse_json_response(&response)?;
    let mut map = expand_to_files(map, &repo_map.files);
    let unplaced = unassigned_files(&map, &repo_map.files);
    if !unplaced.is_empty() {
        let placed = assign_unplaced_files(&mut map, &unplaced, llm).await?;
        tracing::info!(
            placed,
            asked = unplaced.len(),
            "placed files the clustering left unassigned"
        );
    }
    let report = enforce_coverage(&mut map, &all_paths);

    if !report.unknown.is_empty() {
        tracing::warn!(
            paths = ?report.unknown,
            "clustering cited {} unknown path(s), dropped",
            report.unknown.len()
        );
    }
    if !report.overlapping.is_empty() {
        tracing::warn!(
            paths = ?report.overlapping,
            "clustering assigned {} path(s) to more than one domain, kept only the first",
            report.overlapping.len()
        );
    }
    if !report.uncovered.is_empty() {
        tracing::warn!(
            count = report.uncovered.len(),
            "clustering left {} file(s) unassigned, bucketed into \"{UNCATEGORIZED_SLUG}\"",
            report.uncovered.len()
        );
    }

    let layered = layer_named_domains(&map);
    if !layered.is_empty() {
        tracing::warn!(
            domains = ?layered,
            "domains named after a technical layer, not a business concept"
        );
    }

    map.save(repo_root)?;
    prints.domains = Some(input_print);
    prints.save(repo_root)?;
    Ok((map, report))
}
