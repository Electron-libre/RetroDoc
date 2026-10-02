//! Technology and file role identification (PLAN.md §7.1, phase 7 step 1).
//!
//! One LLM call over a compact view of the file tree (plus the root manifest
//! files) identifies the stack and says where entry points, models, business
//! logic, views, infrastructure, config and tests live, as glob rules with a
//! role each. The rules are then applied mechanically to every file (most
//! specific match wins), so the number of relevant files is known before any
//! per-file LLM call. The rules are saved as `.retrodoc/cache/roles.yaml`,
//! reviewable and hand-editable: when the file exists it is reused as is
//! (`force` re-identifies).
//!
//! Files no rule matches stay [`FileRole::Unclassified`]; the planned second
//! pass over just those paths is not implemented yet.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use retrodoc_ingest::{FileEntry, FileKind, IngestResult};
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::error::PipelineError;
use crate::response::complete_json;

const ROLES_RELATIVE_PATH: &str = ".retrodoc/cache/roles.yaml";

/// Manifest files read at the repo root to help recognize the stack.
const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "Gemfile",
    "package.json",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "composer.json",
];
const MAX_MANIFEST_CHARS: usize = 1_500;
/// Directory lines sent to the LLM (shallowest first); deeper ones are
/// summed up in a trailing note.
const MAX_TREE_DIRS: usize = 300;
const SAMPLE_FILES_PER_DIR: usize = 4;

const ROLES_SYSTEM_PROMPT: &str = "You are analyzing the file tree of a software repository. \
First identify the technology stack and its conventions (language, framework, where it puts things). \
Then give glob rules that assign a role to every kind of file. Roles: \"entrypoint\" (routes, \
controllers, CLI commands, jobs, message consumers, webhooks, public API of a library), \"model\" \
(entities, schemas, domain objects, migrations), \"logic\" (services, presenters, policies and other \
business logic), \"view\" (templates, UI components, serializers), \"infra\" (database, queue, \
HTTP clients, deployment, CI), \"config\" (settings, manifests, lockfiles), \"test\", \"docs\", \
\"other\" (assets, vendored or generated files). Patterns use glob syntax relative to the repo \
root, e.g. \"app/models/**\" or \"**/*.erb\"; when several rules match a file the most specific \
pattern wins, so broad fallbacks are fine. Reply with ONLY a single JSON object, no prose and no \
Markdown code fence, matching this shape: {\"stack\":\"one sentence\",\"rules\":[{\"pattern\":\
\"...\",\"role\":\"model\"}]}.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRole {
    #[serde(rename = "entrypoint", alias = "entry_point")]
    EntryPoint,
    Model,
    Logic,
    View,
    Infra,
    Config,
    Test,
    Docs,
    Other,
    /// No rule matched. Also what an unknown role name from the LLM parses to.
    #[serde(other)]
    Unclassified,
}

impl FileRole {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            FileRole::EntryPoint => "entrypoint",
            FileRole::Model => "model",
            FileRole::Logic => "logic",
            FileRole::View => "view",
            FileRole::Infra => "infra",
            FileRole::Config => "config",
            FileRole::Test => "test",
            FileRole::Docs => "docs",
            FileRole::Other => "other",
            FileRole::Unclassified => "unclassified",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleRule {
    pub pattern: String,
    pub role: FileRole,
}

/// The reviewable artifact: the stack as the LLM understood it and the rules
/// derived from it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleRules {
    pub stack: String,
    pub rules: Vec<RoleRule>,
}

/// A role per file, with the rules that produced it.
#[derive(Debug, Clone)]
pub struct RoleMap {
    pub roles: BTreeMap<PathBuf, FileRole>,
}

impl RoleMap {
    /// Number of files per role (roles with no file are absent).
    #[must_use]
    pub fn distribution(&self) -> BTreeMap<FileRole, usize> {
        let mut counts = BTreeMap::new();
        for role in self.roles.values() {
            *counts.entry(*role).or_insert(0) += 1;
        }
        counts
    }

    #[must_use]
    pub fn files_with(&self, role: FileRole) -> Vec<&Path> {
        self.roles
            .iter()
            .filter(|(_, r)| **r == role)
            .map(|(p, _)| p.as_path())
            .collect()
    }
}

impl RoleRules {
    /// Loads `.retrodoc/cache/roles.yaml`. Missing or unreadable: `None`.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(repo_root.join(ROLES_RELATIVE_PATH)).ok()?;
        serde_yaml::from_str(&raw).ok()
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        let path = repo_root.join(ROLES_RELATIVE_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PipelineError::ArtifactIo {
                path: path.clone(),
                source,
            })?;
        }
        let raw = serde_yaml::to_string(self)?;
        std::fs::write(&path, raw).map_err(|source| PipelineError::ArtifactIo { path, source })
    }

    /// Applies the rules to `files`. [`FileKind::Test`] and
    /// [`FileKind::Markdown`] files are fixed mechanically (`Test`/`Docs`);
    /// for the others the matching rule with the most literal characters in
    /// its pattern wins (the first one on a tie). A rule whose pattern isn't
    /// a valid glob is skipped with a warning.
    #[must_use]
    pub fn classify(&self, files: &[FileEntry]) -> RoleMap {
        let mut matchers: Vec<(usize, GlobMatcher, FileRole)> = Vec::new();
        for rule in &self.rules {
            if rule.role == FileRole::Unclassified {
                tracing::warn!(pattern = %rule.pattern, "role rule with an unknown role, ignored");
                continue;
            }
            let pattern = if rule.pattern.ends_with('/') {
                format!("{}**", rule.pattern)
            } else {
                rule.pattern.clone()
            };
            match Glob::new(pattern.trim_start_matches("./")) {
                Ok(glob) => {
                    matchers.push((specificity(&pattern), glob.compile_matcher(), rule.role));
                }
                Err(err) => {
                    tracing::warn!(pattern = %rule.pattern, error = %err, "invalid role rule pattern, ignored");
                }
            }
        }

        let roles = files
            .iter()
            .map(|file| {
                let role = match file.kind {
                    FileKind::Test => FileRole::Test,
                    FileKind::Markdown => FileRole::Docs,
                    FileKind::Source | FileKind::Other => matchers
                        .iter()
                        .filter(|(_, matcher, _)| matcher.is_match(&file.path))
                        .fold(
                            None::<(usize, FileRole)>,
                            |best, (score, _, role)| match best {
                                Some((b, _)) if b >= *score => best,
                                _ => Some((*score, *role)),
                            },
                        )
                        .map_or(FileRole::Unclassified, |(_, role)| role),
                };
                (file.path.clone(), role)
            })
            .collect();
        RoleMap { roles }
    }
}

/// Count of non-wildcard characters: longer literal patterns are more specific.
fn specificity(pattern: &str) -> usize {
    pattern
        .chars()
        .filter(|c| !matches!(c, '*' | '?' | '[' | ']' | '{' | '}' | ','))
        .count()
}

#[derive(Debug, Deserialize)]
struct RolesResponse {
    #[serde(default)]
    stack: String,
    #[serde(default)]
    rules: Vec<RoleRule>,
}

/// Returns the role rules for the repo: the saved `roles.yaml` if there is
/// one (and `force` is false), otherwise the result of one LLM call over the
/// file tree, saved for next time. An unparseable answer yields empty rules
/// (every file unclassified) with a warning, and is not saved.
///
/// # Errors
///
/// Returns an error if the LLM call fails or the rules can't be saved.
pub async fn identify_roles(
    repo_root: &Path,
    ingest: &IngestResult,
    llm: &dyn LlmProvider,
    force: bool,
) -> Result<RoleRules, PipelineError> {
    if !force {
        if let Some(rules) = RoleRules::load(repo_root) {
            return Ok(rules);
        }
    }

    let mut prompt = String::from("File tree (one line per directory):\n");
    prompt.push_str(&render_tree(&ingest.files));
    for name in MANIFESTS {
        if let Ok(content) = std::fs::read_to_string(repo_root.join(name)) {
            let excerpt: String = content.chars().take(MAX_MANIFEST_CHARS).collect();
            let _ = write!(prompt, "\n--- {name} ---\n{excerpt}\n");
        }
    }

    let Some(response) =
        complete_json::<RolesResponse>(llm, ROLES_SYSTEM_PROMPT, &prompt, "file role rules")
            .await?
    else {
        return Ok(RoleRules::default());
    };
    let rules = RoleRules {
        stack: response.stack,
        rules: response.rules,
    };
    rules.save(repo_root)?;
    Ok(rules)
}

/// Compact tree for the prompt: per directory, its file count, extension
/// histogram and a few sample names, so thousands of files stay in budget.
fn render_tree(files: &[FileEntry]) -> String {
    struct Dir<'a> {
        count: usize,
        extensions: BTreeMap<String, usize>,
        samples: Vec<&'a str>,
    }
    let mut dirs: BTreeMap<PathBuf, Dir> = BTreeMap::new();
    for file in files {
        let dir = file.path.parent().unwrap_or(Path::new("")).to_path_buf();
        let entry = dirs.entry(dir).or_insert_with(|| Dir {
            count: 0,
            extensions: BTreeMap::new(),
            samples: Vec::new(),
        });
        entry.count += 1;
        let ext = file.path.extension().map_or_else(
            || "(none)".to_string(),
            |e| format!(".{}", e.to_string_lossy()),
        );
        *entry.extensions.entry(ext).or_insert(0) += 1;
        if entry.samples.len() < SAMPLE_FILES_PER_DIR {
            if let Some(name) = file.path.file_name().and_then(|n| n.to_str()) {
                entry.samples.push(name);
            }
        }
    }

    let mut ordered: Vec<(&PathBuf, &Dir)> = dirs.iter().collect();
    let omitted = ordered.len().saturating_sub(MAX_TREE_DIRS);
    ordered.sort_by_key(|(path, _)| (path.components().count(), *path));
    ordered.truncate(MAX_TREE_DIRS);
    ordered.sort_by_key(|(path, _)| *path);

    let mut out = String::new();
    for (path, dir) in ordered {
        let label = if path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            format!("{}/", path.display())
        };
        let extensions = dir
            .extensions
            .iter()
            .map(|(ext, n)| format!("{ext}×{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(
            out,
            "{label} — {} file(s) [{extensions}] e.g. {}",
            dir.count,
            dir.samples.join(", ")
        );
    }
    if omitted > 0 {
        let _ = writeln!(out, "… and {omitted} deeper directories not shown");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;
    use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingProvider {
        response: String,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl LlmProvider for CountingProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CompletionResponse {
                content: self.response.clone(),
                model: "test-model".to_string(),
            })
        }
    }

    fn entry(path: &str, kind: FileKind) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            kind,
            size_bytes: 1,
        }
    }

    fn rules(list: &[(&str, FileRole)]) -> RoleRules {
        RoleRules {
            stack: "test".to_string(),
            rules: list
                .iter()
                .map(|(pattern, role)| RoleRule {
                    pattern: (*pattern).to_string(),
                    role: *role,
                })
                .collect(),
        }
    }

    #[test]
    fn most_specific_rule_wins() {
        let rules = rules(&[
            ("app/**", FileRole::Logic),
            ("app/models/**", FileRole::Model),
            ("**/*.erb", FileRole::View),
            ("app/controllers/", FileRole::EntryPoint),
        ]);
        let map = rules.classify(&[
            entry("app/models/user.rb", FileKind::Source),
            entry("app/services/pay.rb", FileKind::Source),
            entry("app/models/show.html.erb", FileKind::Other),
            entry("app/controllers/a/b.rb", FileKind::Source),
            entry("lib/x.rb", FileKind::Source),
        ]);
        let role = |p: &str| map.roles[Path::new(p)];
        assert_eq!(role("app/models/user.rb"), FileRole::Model);
        assert_eq!(role("app/services/pay.rb"), FileRole::Logic);
        assert_eq!(role("app/models/show.html.erb"), FileRole::Model);
        assert_eq!(role("app/controllers/a/b.rb"), FileRole::EntryPoint);
        assert_eq!(role("lib/x.rb"), FileRole::Unclassified);
    }

    #[test]
    fn tests_and_docs_are_fixed_and_bad_rules_skipped() {
        let rules = rules(&[("**", FileRole::Logic), ("a[", FileRole::Model)]);
        let map = rules.classify(&[
            entry("spec/a_spec.rb", FileKind::Test),
            entry("README.md", FileKind::Markdown),
            entry("a.rb", FileKind::Source),
        ]);
        assert_eq!(map.roles[Path::new("spec/a_spec.rb")], FileRole::Test);
        assert_eq!(map.roles[Path::new("README.md")], FileRole::Docs);
        assert_eq!(map.roles[Path::new("a.rb")], FileRole::Logic);
        let dist = map.distribution();
        assert_eq!(dist[&FileRole::Logic], 1);
        assert_eq!(map.files_with(FileRole::Test).len(), 1);
    }

    #[test]
    fn entrypoint_role_round_trips() {
        let parsed: RolesResponse = serde_json::from_str(
            r#"{"rules":[{"pattern":"a","role":"entrypoint"},{"pattern":"b","role":"entry_point"}]}"#,
        )
        .unwrap();
        assert!(parsed.rules.iter().all(|r| r.role == FileRole::EntryPoint));
        assert!(serde_yaml::to_string(&parsed.rules[0].role)
            .unwrap()
            .contains("entrypoint"));
    }

    #[test]
    fn unknown_role_name_parses_as_unclassified() {
        let parsed: RolesResponse =
            serde_json::from_str(r#"{"rules":[{"pattern":"x/**","role":"banana"}]}"#).unwrap();
        assert_eq!(parsed.rules[0].role, FileRole::Unclassified);
    }

    #[test]
    fn tree_is_compact_and_bounded() {
        let files: Vec<FileEntry> = (0..400)
            .map(|i| entry(&format!("d{i}/sub/f.rb"), FileKind::Source))
            .chain((0..10).map(|i| entry(&format!("app/f{i}.rb"), FileKind::Source)))
            .collect();
        let tree = render_tree(&files);
        assert!(tree.contains("app/ — 10 file(s) [.rb×10] e.g. f0.rb, f1.rb, f2.rb, f3.rb"));
        assert!(tree.contains("deeper directories not shown"));
        assert!(tree.lines().count() <= MAX_TREE_DIRS + 1);
    }

    #[tokio::test]
    async fn identifies_once_then_reuses_saved_rules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let ingest = IngestResult {
            files: vec![entry("src/lib.rs", FileKind::Source)],
            history_by_path: std::collections::HashMap::new(),
            existing_docs: Vec::new(),
        };
        let llm = CountingProvider {
            response: r#"```json
{"stack":"Rust library","rules":[{"pattern":"src/**","role":"logic"}]}
```"#
                .to_string(),
            calls: AtomicUsize::new(0),
        };

        let first = identify_roles(dir.path(), &ingest, &llm, false)
            .await
            .unwrap();
        assert_eq!(first.stack, "Rust library");
        let second = identify_roles(dir.path(), &ingest, &llm, false)
            .await
            .unwrap();
        assert_eq!(second.rules.len(), 1);
        assert_eq!(llm.calls.load(Ordering::SeqCst), 1);

        identify_roles(dir.path(), &ingest, &llm, true)
            .await
            .unwrap();
        assert_eq!(llm.calls.load(Ordering::SeqCst), 2);
    }
}
