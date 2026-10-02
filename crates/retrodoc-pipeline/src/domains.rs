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
//! - A source file the LLM never assigned anywhere is bucketed into a
//!   synthetic "uncategorized" domain (100% coverage, PLAN.md §2 step 3).
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
use retrodoc_llm::{ChatMessage, CompletionRequest, LlmProvider, Role};
use serde::{Deserialize, Serialize};

use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, Fingerprints};
use crate::repo_map::{FileSummary, RepoMap};
use crate::response::parse_json_response;
use crate::surface::Surface;

const DOMAINS_RELATIVE_PATH: &str = ".retrodoc/cache/domains.yaml";
pub(crate) const UNCATEGORIZED_SLUG: &str = "uncategorized";

const DOMAIN_CLUSTERING_SYSTEM_PROMPT: &str =
    "You are analyzing a software repository to identify \
its functional (business) domains, not its technical/folder structure. Given per-module (folder) \
role summaries, and hints from any existing documentation, group every listed module into a small \
set of functional domains, optionally split into sub-domains when a domain is large enough to \
warrant it. Every module path given to you MUST appear in exactly one domain (or one of its \
sub-domains), copied verbatim; copy the root module's path as \"\" (empty string), never \".\" or \
\"/\". A parent module and one of its child modules may be assigned to different domains: a file \
belongs to whichever of its assigned ancestor modules is most specific (deepest), so it's fine to \
carve a sub-directory out into its own domain while leaving the parent's remaining files under the \
parent's domain. Reply with ONLY a single JSON object, no prose and no Markdown code fence, \
matching this shape: {\"domains\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\"description\":\
\"...\",\"paths\":[\"...\"],\"sub_domains\":[{\"slug\":\"...\",\"name\":\"...\",\"description\":\
\"...\",\"paths\":[\"...\"]}]}]}. A domain's `paths` holds only the modules not better placed in \
one of its `sub_domains`.";

/// Added to the system prompt when the application surface (entities, entry
/// points) is available: the domains must read as business, not as layers.
const BUSINESS_NAMING_ADDENDUM: &str = " The prompt also lists the application's business \
entities (from its data models) and its entry points grouped by resource. Use them to decide which \
modules belong together: name each domain after a business concept or activity (e.g. contract \
signing, company onboarding, billing), and put each module with the concept it serves most. Never \
name a domain after a technical layer or folder kind (presentation, services, models, controllers, \
views, helpers, utils, infrastructure, api, jobs).";

/// Words that name a technical layer rather than a business domain.
const LAYER_WORDS: &[&str] = &[
    "presentation",
    "presenter",
    "presenters",
    "service",
    "services",
    "model",
    "models",
    "controller",
    "controllers",
    "view",
    "views",
    "helper",
    "helpers",
    "util",
    "utils",
    "utility",
    "utilities",
    "infrastructure",
    "api",
    "job",
    "jobs",
    "layer",
    "lib",
    "core",
    "common",
    "shared",
    "misc",
    "other",
];

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
        let path = repo_root.join(DOMAINS_RELATIVE_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PipelineError::DomainsIo {
                path: path.clone(),
                source,
            })?;
        }
        let raw = serde_yaml::to_string(self)?;
        std::fs::write(&path, raw).map_err(|source| PipelineError::DomainsIo { path, source })?;
        Ok(())
    }

    /// Loads a previously saved `domains.yaml`. Missing or unreadable:
    /// `None` (first run), not an error.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        let path = repo_root.join(DOMAINS_RELATIVE_PATH);
        let raw = std::fs::read_to_string(path).ok()?;
        serde_yaml::from_str(&raw).ok()
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
    let response = llm
        .complete(CompletionRequest {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: system_prompt,
                },
                ChatMessage {
                    role: Role::User,
                    content: prompt,
                },
            ],
            model: None,
        })
        .await?;

    let map: DomainMap = parse_json_response(&response.content)?;
    let mut map = expand_to_files(map, &repo_map.files);
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

/// Hash of what the clustering is derived from. Directory summaries are left
/// out on purpose: they are regenerated by the LLM on every run, so they
/// would change the hash while the files did not.
fn clustering_fingerprint(
    repo_map: &RepoMap,
    existing_docs: &[ExistingDoc],
    surface: &Surface,
) -> String {
    let files = repo_map
        .files
        .iter()
        .map(|f| format!("{}\n{}", f.path.display(), f.role_summary));
    let docs = existing_docs
        .iter()
        .map(|d| format!("{}\n{}", d.path.display(), d.content));
    fingerprint(files.chain(docs).chain(surface.fingerprint_parts()))
}

/// Slugs of the domains and sub-domains whose name is made only of layer
/// words (`presentation-layer`, `services`): the technical split PLAN.md §6
/// warns about. Reported, not repaired.
fn layer_named_domains(map: &DomainMap) -> Vec<String> {
    let is_layer = |slug: &str| {
        slug.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .all(|w| LAYER_WORDS.contains(&w.to_lowercase().as_str()))
    };
    let mut found = Vec::new();
    for domain in &map.domains {
        if domain.slug != UNCATEGORIZED_SLUG && is_layer(&domain.slug) {
            found.push(domain.slug.clone());
        }
        for sub in &domain.sub_domains {
            if is_layer(&sub.slug) {
                found.push(format!("{}/{}", domain.slug, sub.slug));
            }
        }
    }
    found
}

/// One directory the LLM assigned to a domain (or sub-domain), with its
/// depth precomputed for longest-prefix-match resolution.
struct DirAssignment {
    dir: PathBuf,
    depth: usize,
    domain_idx: usize,
    sub_domain_idx: Option<usize>,
}

/// Flattens every directory `map` assigned (domain `paths` first, then each
/// domain's `sub_domains` `paths`, in order) into a list of
/// [`DirAssignment`]s for [`resolve_target`].
fn collect_dir_assignments(map: &DomainMap) -> Vec<DirAssignment> {
    let mut assignments = Vec::new();
    for (domain_idx, domain) in map.domains.iter().enumerate() {
        for dir in &domain.paths {
            assignments.push(DirAssignment {
                dir: dir.clone(),
                depth: dir.components().count(),
                domain_idx,
                sub_domain_idx: None,
            });
        }
        for (sub_domain_idx, sub) in domain.sub_domains.iter().enumerate() {
            for dir in &sub.paths {
                assignments.push(DirAssignment {
                    dir: dir.clone(),
                    depth: dir.components().count(),
                    domain_idx,
                    sub_domain_idx: Some(sub_domain_idx),
                });
            }
        }
    }
    assignments
}

/// Picks the assigned directory that is the deepest (most specific) ancestor
/// of `file_path`, i.e. the longest-prefix match. Ties (only possible when
/// the LLM assigned the same directory twice) resolve to the first
/// occurrence in `assignments`.
fn resolve_target<'a>(
    assignments: &'a [DirAssignment],
    file_path: &Path,
) -> Option<&'a DirAssignment> {
    let mut best: Option<&DirAssignment> = None;
    for candidate in assignments.iter().filter(|a| file_path.starts_with(&a.dir)) {
        if best.is_none_or(|b| candidate.depth > b.depth) {
            best = Some(candidate);
        }
    }
    best
}

/// Mechanically expands a module/directory-level clustering into a
/// file-level one: each file is routed to its most-specific assigned
/// ancestor directory (see module docs). Directory assignments that match no
/// real file become domains/sub-domains with empty `paths`, which is
/// harmless. A file matched by no assigned directory is left unassigned;
/// the caller's subsequent [`enforce_coverage`] call buckets it into
/// "uncategorized" exactly as it would a file an LLM forgot under the old
/// flat per-file scheme.
fn expand_to_files(map: DomainMap, files: &[FileSummary]) -> DomainMap {
    let assignments = collect_dir_assignments(&map);

    let mut expanded = DomainMap {
        domains: map
            .domains
            .into_iter()
            .map(|domain| DomainCluster {
                paths: Vec::new(),
                sub_domains: domain
                    .sub_domains
                    .into_iter()
                    .map(|sub| SubDomainCluster {
                        paths: Vec::new(),
                        ..sub
                    })
                    .collect(),
                ..domain
            })
            .collect(),
    };

    for file in files {
        let Some(target) = resolve_target(&assignments, &file.path) else {
            continue;
        };
        match target.sub_domain_idx {
            Some(sub_idx) => expanded.domains[target.domain_idx].sub_domains[sub_idx]
                .paths
                .push(file.path.clone()),
            None => expanded.domains[target.domain_idx]
                .paths
                .push(file.path.clone()),
        }
    }

    expanded
}

/// Enforces 100% coverage and no overlap on `map` in place (see module
/// docs), and reports what was found/repaired.
fn enforce_coverage(map: &mut DomainMap, all_paths: &[PathBuf]) -> CoverageReport {
    let known: BTreeSet<&PathBuf> = all_paths.iter().collect();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut overlapping = Vec::new();
    let mut unknown = Vec::new();

    for domain in &mut map.domains {
        retain_known_and_first_seen(
            &mut domain.paths,
            &known,
            &mut seen,
            &mut overlapping,
            &mut unknown,
        );
        for sub in &mut domain.sub_domains {
            retain_known_and_first_seen(
                &mut sub.paths,
                &known,
                &mut seen,
                &mut overlapping,
                &mut unknown,
            );
        }
    }

    let uncovered: Vec<PathBuf> = all_paths
        .iter()
        .filter(|p| !seen.contains(*p))
        .cloned()
        .collect();
    if !uncovered.is_empty() {
        map.domains.push(uncategorized_domain(uncovered.clone()));
    }

    CoverageReport {
        uncovered,
        overlapping,
        unknown,
    }
}

/// Keeps a path only if it's a known source file and its first appearance
/// across the whole clustering; drops duplicates and hallucinated paths
/// into `overlapping`/`unknown` respectively.
fn retain_known_and_first_seen(
    paths: &mut Vec<PathBuf>,
    known: &BTreeSet<&PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
    overlapping: &mut Vec<PathBuf>,
    unknown: &mut Vec<PathBuf>,
) {
    paths.retain(|path| {
        if !known.contains(path) {
            unknown.push(path.clone());
            return false;
        }
        if seen.insert(path.clone()) {
            true
        } else {
            overlapping.push(path.clone());
            false
        }
    });
}

fn uncategorized_domain(paths: Vec<PathBuf>) -> DomainCluster {
    DomainCluster {
        slug: UNCATEGORIZED_SLUG.to_string(),
        name: "Uncategorized".to_string(),
        description: "Files the clustering pass could not confidently assign to a functional \
            domain; needs manual review."
            .to_string(),
        paths,
        sub_domains: Vec::new(),
    }
}

fn clustering_prompt(
    repo_map: &RepoMap,
    existing_docs: &[ExistingDoc],
    surface: &Surface,
) -> String {
    let mut prompt = String::new();
    if !surface.is_empty() {
        prompt.push_str(&surface.prompt_section());
        prompt.push('\n');
    }
    let _ = writeln!(prompt, "Modules:");
    for module in &repo_map.modules {
        let label = if module.path.as_os_str().is_empty() {
            String::new()
        } else {
            module.path.display().to_string()
        };
        let _ = writeln!(
            prompt,
            "- \"{label}\" ({} files): {}",
            module.file_count, module.role_summary
        );
    }

    if !existing_docs.is_empty() {
        let _ = writeln!(
            prompt,
            "\nExisting documentation (titles only, hints on business vocabulary):"
        );
        for doc in existing_docs {
            let title = doc
                .content
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("");
            let _ = writeln!(prompt, "- {}: {title}", doc.path.display());
        }
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use async_trait::async_trait;
    use retrodoc_ingest::{FileEntry, FileKind, IngestResult};
    use retrodoc_llm::{CompletionResponse, LlmError};

    use crate::repo_map::{build_repo_map, FileSummary, ModuleSummary};

    /// Fake provider that always answers a fixed clustering response, to
    /// test parsing + coverage enforcement without depending on the
    /// network.
    struct CannedProvider {
        response: String,
    }

    #[async_trait]
    impl LlmProvider for CannedProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            Ok(CompletionResponse {
                content: self.response.clone(),
                model: "test-model".to_string(),
            })
        }
    }

    /// Provider that panics if called, to assert a fast path never reaches
    /// the LLM.
    struct PanicProvider;

    #[async_trait]
    impl LlmProvider for PanicProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            panic!("LLM should not have been called");
        }
    }

    fn file_summary(path: &str) -> FileSummary {
        FileSummary {
            path: PathBuf::from(path),
            role_summary: format!("role of {path}"),
            commit_count: 1,
            author_count: 1,
        }
    }

    #[test]
    fn enforce_coverage_dedupes_overlap_drops_unknown_and_buckets_uncovered() {
        let mut map = DomainMap {
            domains: vec![DomainCluster {
                slug: "billing".to_string(),
                name: "Billing".to_string(),
                description: "d".to_string(),
                paths: vec![PathBuf::from("a.rs"), PathBuf::from("ghost.rs")],
                sub_domains: vec![SubDomainCluster {
                    slug: "invoices".to_string(),
                    name: "Invoices".to_string(),
                    description: "d".to_string(),
                    // "a.rs" is a duplicate (already claimed above).
                    paths: vec![PathBuf::from("a.rs")],
                }],
            }],
        };
        let all_paths = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];

        let report = enforce_coverage(&mut map, &all_paths);

        assert_eq!(report.overlapping, vec![PathBuf::from("a.rs")]);
        assert_eq!(report.unknown, vec![PathBuf::from("ghost.rs")]);
        assert_eq!(report.uncovered, vec![PathBuf::from("b.rs")]);
        assert!(!report.is_clean());

        // "b.rs" landed in the synthetic uncategorized domain.
        let uncategorized = map
            .domains
            .iter()
            .find(|d| d.slug == UNCATEGORIZED_SLUG)
            .unwrap();
        assert_eq!(uncategorized.paths, vec![PathBuf::from("b.rs")]);

        // "a.rs" only appears once in the repaired map.
        let billing = &map.domains[0];
        assert_eq!(billing.paths, vec![PathBuf::from("a.rs")]);
        assert!(billing.sub_domains[0].paths.is_empty());
    }

    #[test]
    fn clean_map_reports_nothing() {
        let mut map = DomainMap {
            domains: vec![DomainCluster {
                slug: "billing".to_string(),
                name: "Billing".to_string(),
                description: "d".to_string(),
                paths: vec![PathBuf::from("a.rs")],
                sub_domains: vec![],
            }],
        };
        let all_paths = vec![PathBuf::from("a.rs")];

        let report = enforce_coverage(&mut map, &all_paths);

        assert!(report.is_clean());
        assert!(!map.domains.iter().any(|d| d.slug == UNCATEGORIZED_SLUG));
    }

    #[tokio::test]
    async fn build_domains_skips_the_llm_when_there_are_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap::default();

        let (map, report) = build_domains(
            dir.path(),
            &repo_map,
            &[],
            &Surface::default(),
            &PanicProvider,
        )
        .await
        .unwrap();

        assert!(map.domains.is_empty());
        assert!(report.is_clean());
    }

    #[tokio::test]
    async fn build_domains_parses_response_and_persists_the_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap {
            files: vec![file_summary("a.rs"), file_summary("b.rs")],
            modules: vec![ModuleSummary {
                path: PathBuf::new(),
                role_summary: "root".to_string(),
                file_count: 2,
            }],
        };
        let provider = CannedProvider {
            response: r#"```json
            {"domains":[{"slug":"billing","name":"Billing","description":"Handles invoices.",
            "paths":[""],"sub_domains":[]}]}
            ```"#
                .to_string(),
        };

        let (map, report) =
            build_domains(dir.path(), &repo_map, &[], &Surface::default(), &provider)
                .await
                .unwrap();

        assert!(report.is_clean());
        assert_eq!(map.domains.len(), 1);
        assert_eq!(map.domains[0].slug, "billing");
        assert_eq!(
            map.domains[0].paths,
            vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")]
        );

        let reloaded = DomainMap::load(dir.path()).unwrap();
        assert_eq!(reloaded.domains.len(), 1);
        assert_eq!(reloaded.domains[0].slug, "billing");
    }

    #[tokio::test]
    async fn build_domains_reuses_the_saved_clustering_when_the_input_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap {
            files: vec![file_summary("a.rs")],
            modules: vec![],
        };
        let provider = CannedProvider {
            response: r#"{"domains":[{"slug":"billing","name":"Billing","description":"d",
            "paths":["a.rs"],"sub_domains":[]}]}"#
                .to_string(),
        };
        build_domains(dir.path(), &repo_map, &[], &Surface::default(), &provider)
            .await
            .unwrap();

        // Same input: the LLM must not be called again.
        let (map, _) = build_domains(
            dir.path(),
            &repo_map,
            &[],
            &Surface::default(),
            &PanicProvider,
        )
        .await
        .unwrap();
        assert_eq!(map.domains[0].slug, "billing");

        // Changed input: it is.
        let changed = RepoMap {
            files: vec![file_summary("a.rs"), file_summary("b.rs")],
            modules: vec![],
        };
        let (map, _) = build_domains(dir.path(), &changed, &[], &Surface::default(), &provider)
            .await
            .unwrap();
        assert_eq!(map.domains[0].paths.len(), 1); // canned answer, b.rs uncovered
        assert!(map.domains.iter().any(|d| d.slug == UNCATEGORIZED_SLUG));
    }

    #[tokio::test]
    async fn build_domains_works_end_to_end_with_a_real_repo_map() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();

        let ingest = IngestResult {
            files: vec![FileEntry {
                path: PathBuf::from("a.rs"),
                kind: FileKind::Source,
                size_bytes: 9,
            }],
            history_by_path: HashMap::new(),
            existing_docs: Vec::new(),
        };

        let repo_map_provider = CannedProvider {
            response: "a summary".to_string(),
        };
        let repo_map = build_repo_map(dir.path(), &ingest, &repo_map_provider, 1)
            .await
            .unwrap();

        let clustering_provider = CannedProvider {
            response: r#"{"domains":[{"slug":"core","name":"Core","description":"d",
            "paths":[""],"sub_domains":[]}]}"#
                .to_string(),
        };
        let (map, report) = build_domains(
            dir.path(),
            &repo_map,
            &[],
            &Surface::default(),
            &clustering_provider,
        )
        .await
        .unwrap();

        assert!(report.is_clean());
        assert_eq!(map.domains[0].paths, vec![PathBuf::from("a.rs")]);
    }

    fn domain_with_dirs(slug: &str, dirs: &[&str]) -> DomainCluster {
        DomainCluster {
            slug: slug.to_string(),
            name: slug.to_string(),
            description: "d".to_string(),
            paths: dirs.iter().map(PathBuf::from).collect(),
            sub_domains: Vec::new(),
        }
    }

    #[test]
    fn expand_to_files_resolves_longest_prefix_match() {
        let map = DomainMap {
            domains: vec![
                domain_with_dirs("backend", &["src"]),
                domain_with_dirs("frontend", &["src/ui"]),
            ],
        };
        let files = vec![
            file_summary("src/main.rs"),
            file_summary("src/ui/button.rs"),
        ];

        let expanded = expand_to_files(map, &files);

        assert_eq!(
            expanded.domains[0].paths,
            vec![PathBuf::from("src/main.rs")]
        );
        assert_eq!(
            expanded.domains[1].paths,
            vec![PathBuf::from("src/ui/button.rs")]
        );
    }

    #[test]
    fn expand_to_files_uses_root_as_fallback_only_when_nothing_more_specific_wins() {
        let map = DomainMap {
            domains: vec![
                domain_with_dirs("core", &[""]),
                domain_with_dirs("docs", &["docs"]),
            ],
        };
        let files = vec![file_summary("lib.rs"), file_summary("docs/helper.rs")];

        let expanded = expand_to_files(map, &files);

        assert_eq!(expanded.domains[0].paths, vec![PathBuf::from("lib.rs")]);
        assert_eq!(
            expanded.domains[1].paths,
            vec![PathBuf::from("docs/helper.rs")]
        );
    }

    #[test]
    fn expand_to_files_tolerates_a_directory_matching_no_real_file() {
        let map = DomainMap {
            domains: vec![
                domain_with_dirs("ghost-hunters", &["ghost/dir"]),
                domain_with_dirs("real", &["src"]),
            ],
        };
        let files = vec![file_summary("src/main.rs")];

        let expanded = expand_to_files(map, &files);

        assert!(expanded.domains[0].paths.is_empty());
        assert_eq!(
            expanded.domains[1].paths,
            vec![PathBuf::from("src/main.rs")]
        );
    }

    #[test]
    fn expand_to_files_resolves_duplicate_directory_claim_to_the_first_domain() {
        let map = DomainMap {
            domains: vec![
                domain_with_dirs("first", &["src"]),
                domain_with_dirs("second", &["src"]),
            ],
        };
        let files = vec![file_summary("src/main.rs")];

        let expanded = expand_to_files(map, &files);

        assert_eq!(
            expanded.domains[0].paths,
            vec![PathBuf::from("src/main.rs")]
        );
        assert!(expanded.domains[1].paths.is_empty());
    }

    #[test]
    fn clustering_prompt_renders_root_module_as_empty_string_not_dot() {
        let repo_map = RepoMap {
            files: vec![],
            modules: vec![
                ModuleSummary {
                    path: PathBuf::new(),
                    role_summary: "root".to_string(),
                    file_count: 3,
                },
                ModuleSummary {
                    path: PathBuf::from("src"),
                    role_summary: "source".to_string(),
                    file_count: 2,
                },
            ],
        };

        let prompt = clustering_prompt(&repo_map, &[], &Surface::default());

        assert!(prompt.contains("- \"\" "));
        assert!(!prompt
            .lines()
            .any(|l| l.trim() == "- ." || l.trim().starts_with("- .:")));
    }

    #[test]
    fn flags_domains_named_after_a_technical_layer() {
        let cluster = |slug: &str| DomainCluster {
            slug: slug.to_string(),
            name: slug.to_string(),
            description: String::new(),
            paths: Vec::new(),
            sub_domains: Vec::new(),
        };
        let map = DomainMap {
            domains: vec![
                cluster("presentation-layer"),
                cluster("contract-signing"),
                cluster("services"),
                cluster("uncategorized"),
            ],
        };
        assert_eq!(
            layer_named_domains(&map),
            vec!["presentation-layer", "services"]
        );
    }

    /// Records the prompts it receives.
    struct RecordingProvider {
        prompts: std::sync::Mutex<Vec<(String, String)>>,
    }

    #[async_trait]
    impl LlmProvider for RecordingProvider {
        async fn complete(
            &self,
            request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.prompts.lock().unwrap().push((
                request.messages[0].content.clone(),
                request.messages[1].content.clone(),
            ));
            Ok(CompletionResponse {
                content: r#"{"domains":[{"slug":"contracts","name":"Contracts","description":"d","paths":[""]}]}"#
                    .to_string(),
                model: "m".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn the_surface_reaches_the_prompt_and_invalidates_the_saved_clustering() {
        use crate::entry_points::EntryPoints;
        use crate::glossary::{Entity, Glossary, ModelFile};

        let dir = tempfile::tempdir().unwrap();
        let repo_map = RepoMap {
            files: vec![file_summary("a.rs")],
            modules: vec![ModuleSummary {
                path: PathBuf::new(),
                role_summary: "root".to_string(),
                file_count: 1,
            }],
        };
        let glossary = Glossary {
            models: std::collections::BTreeMap::from([(
                PathBuf::from("a.rs"),
                ModelFile {
                    content_hash: String::new(),
                    entities: vec![Entity {
                        name: "Contract".to_string(),
                        description: "An agreement".to_string(),
                        attributes: Vec::new(),
                        associations: Vec::new(),
                    }],
                },
            )]),
            tests: Vec::new(),
        };
        let surface = Surface::new(&glossary, &EntryPoints::default());
        let provider = RecordingProvider {
            prompts: std::sync::Mutex::new(Vec::new()),
        };

        // Without a surface: the plain prompt. With one: entities + naming rule.
        build_domains(dir.path(), &repo_map, &[], &Surface::default(), &provider)
            .await
            .unwrap();
        build_domains(dir.path(), &repo_map, &[], &surface, &provider)
            .await
            .unwrap();

        let prompts = provider.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2, "a new surface must recompute the domains");
        assert!(!prompts[0].0.contains("technical layer"));
        assert!(!prompts[0].1.contains("Business entities"));
        assert!(prompts[1]
            .0
            .contains("Never name a domain after a technical layer"));
        assert!(prompts[1].1.contains("- Contract (a.rs): An agreement"));
    }
}
