//! Entry points and outputs inventory (PLAN.md §7.1, phase 7 step 3).
//!
//! Every entry point of the application (HTTP route or controller action,
//! CLI command, job, consumer, webhook, or the public API of a library) is a
//! use-case candidate, and its outputs (response, email, generated file,
//! emitted event, external call, database write) are the observable effects
//! of that use case. The LLM reads only the files the role rules classified
//! as [`FileRole::EntryPoint`], several small files per call, and the result
//! is cached by content hash in `.retrodoc/cache/entry-points.yaml`, like the
//! glossary.
//!
//! The same entry point can be listed twice from two angles (the route in a
//! routes file, the action in its controller); linking them is left to the
//! use-case rewiring (step 4), which traces the code from an entry point.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::glossary::batches;
use crate::progress::Progress;
use crate::repo_map::{read_file_lossy, truncate_chars};
use crate::response::complete_json;
use crate::roles::{FileRole, RoleMap};
use crate::use_cases::resolve_cited_path;

const ENTRY_POINTS_RELATIVE_PATH: &str = ".retrodoc/cache/entry-points.yaml";

/// Entry point files carry many small actions: a bit more room than models.
const MAX_ENTRY_FILE_CHARS: usize = 5_000;

const ENTRY_POINTS_SYSTEM_PROMPT: &str = "You are inventorying the entry points of a software \
application from the files that define them: HTTP routes and controller actions, CLI commands, \
background jobs and schedulers, message consumers, webhooks, or, for a library, its public API. \
List the entry points defined in each file (for a routes file, the routes with their HTTP verb \
and resource). For each give: `kind` (http_route, cli_command, job, consumer, webhook, \
public_api or other); `name` as a reader would say it (e.g. \"POST /contracts/:id/sign\" or \
\"SendReminderJob\"); `verb`, the action in business words (e.g. \"sign\"); `resource`, the \
business object it acts on (e.g. \"contract\"); `description`, one sentence on what it does for \
the user or the business; and `outputs`, its observable effects, each with a `kind` (response, \
email, file, event, external_call, db_write or other) and a short `description`. Skip purely \
technical helpers. Reply with ONLY a single JSON object, no prose and no Markdown code fence, \
matching this shape: {\"entry_points\":[{\"file\":\"path as given\",\"kind\":\"http_route\",\
\"name\":\"...\",\"verb\":\"...\",\"resource\":\"...\",\"description\":\"...\",\"outputs\":\
[{\"kind\":\"email\",\"description\":\"...\"}]}]}. Use the file paths exactly as given.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    HttpRoute,
    CliCommand,
    Job,
    Consumer,
    Webhook,
    PublicApi,
    /// Also what an unknown kind from the LLM parses to.
    #[serde(other)]
    Other,
}

impl EntryKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            EntryKind::HttpRoute => "http_route",
            EntryKind::CliCommand => "cli_command",
            EntryKind::Job => "job",
            EntryKind::Consumer => "consumer",
            EntryKind::Webhook => "webhook",
            EntryKind::PublicApi => "public_api",
            EntryKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Response,
    Email,
    File,
    Event,
    ExternalCall,
    DbWrite,
    /// Also what an unknown kind from the LLM parses to.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    pub kind: OutputKind,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryPoint {
    pub kind: EntryKind,
    pub name: String,
    #[serde(default)]
    pub verb: String,
    #[serde(default)]
    pub resource: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub outputs: Vec<Output>,
}

/// What was read from one entry point file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryFile {
    pub content_hash: String,
    pub entry_points: Vec<EntryPoint>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntryPoints {
    #[serde(default)]
    pub files: BTreeMap<PathBuf, EntryFile>,
}

impl EntryPoints {
    /// Missing or unreadable: `None` (first run).
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(repo_root.join(ENTRY_POINTS_RELATIVE_PATH)).ok()?;
        serde_yaml::from_str(&raw).ok()
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        let path = repo_root.join(ENTRY_POINTS_RELATIVE_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PipelineError::ArtifactIo {
                path: path.clone(),
                source,
            })?;
        }
        let raw = serde_yaml::to_string(self)?;
        std::fs::write(&path, raw).map_err(|source| PipelineError::ArtifactIo { path, source })
    }

    /// Every entry point with the file it was read from, in path order.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &EntryPoint)> {
        self.files
            .iter()
            .flat_map(|(path, file)| file.entry_points.iter().map(move |e| (path.as_path(), e)))
    }

    /// Number of entry points per kind (kinds with none are absent).
    #[must_use]
    pub fn distribution(&self) -> BTreeMap<EntryKind, usize> {
        let mut counts = BTreeMap::new();
        for (_, entry) in self.iter() {
            *counts.entry(entry.kind).or_insert(0) += 1;
        }
        counts
    }
}

#[derive(Debug, Deserialize)]
struct EntryPointsResponse {
    #[serde(default)]
    entry_points: Vec<ResponseEntry>,
}

#[derive(Debug, Deserialize)]
struct ResponseEntry {
    #[serde(default)]
    file: String,
    #[serde(flatten)]
    entry: EntryPoint,
}

/// Builds the entry points inventory from the files classified
/// [`FileRole::EntryPoint`]. Only files new or changed since the saved
/// inventory go to the LLM, in batches; a batch whose answer is unusable is
/// skipped with a warning and its files are retried next run.
///
/// # Errors
///
/// Returns an error if a file can't be read, an LLM call fails (nothing is
/// saved then), or the inventory can't be saved.
pub async fn build_entry_points(
    repo_root: &Path,
    roles: &RoleMap,
    llm: &dyn LlmProvider,
) -> Result<EntryPoints, PipelineError> {
    let previous = EntryPoints::load(repo_root).unwrap_or_default();
    let mut files: BTreeMap<PathBuf, EntryFile> = BTreeMap::new();
    let mut pending: Vec<(PathBuf, String, String)> = Vec::new();

    for path in roles.files_with(FileRole::EntryPoint) {
        let content = read_file_lossy(repo_root, path)?;
        let hash = hash_content(&content);
        match previous.files.get(path) {
            Some(saved) if saved.content_hash == hash => {
                files.insert(path.to_path_buf(), saved.clone());
            }
            _ => pending.push((
                path.to_path_buf(),
                hash,
                truncate_chars(&content, MAX_ENTRY_FILE_CHARS),
            )),
        }
    }

    let batches = batches(&pending);
    let mut progress = Progress::new("entry points", batches.len());
    for batch in batches {
        progress.begin(&format!(
            "{} file(s), from {}",
            batch.len(),
            batch[0].0.display()
        ));
        let mut prompt = String::from("Files:\n");
        for (path, _, content) in batch {
            let _ = write!(prompt, "\n--- {} ---\n{content}\n", path.display());
        }
        let Some(response) = complete_json::<EntryPointsResponse>(
            llm,
            ENTRY_POINTS_SYSTEM_PROMPT,
            &prompt,
            "entry points",
        )
        .await?
        else {
            continue;
        };

        let allowed: BTreeSet<String> = batch
            .iter()
            .map(|(path, _, _)| path.to_string_lossy().into_owned())
            .collect();
        let mut found: BTreeMap<String, Vec<EntryPoint>> = BTreeMap::new();
        for item in response.entry_points {
            let target = if batch.len() == 1 {
                allowed.iter().next().cloned()
            } else {
                resolve_cited_path(&item.file, &allowed)
            };
            match target {
                Some(file) if !item.entry.name.trim().is_empty() => {
                    found.entry(file).or_default().push(item.entry);
                }
                Some(_) => {}
                None => tracing::warn!(
                    file = %item.file,
                    entry_point = %item.entry.name,
                    "entry point attributed to an unknown file, dropped"
                ),
            }
        }
        for (path, hash, _) in batch {
            let entry_points = found
                .remove(path.to_string_lossy().as_ref())
                .unwrap_or_default();
            files.insert(
                path.clone(),
                EntryFile {
                    content_hash: hash.clone(),
                    entry_points,
                },
            );
        }
    }

    let inventory = EntryPoints { files };
    inventory.save(repo_root)?;
    Ok(inventory)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    use async_trait::async_trait;
    use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError};

    struct ScriptedProvider {
        response: String,
        prompts: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl LlmProvider for ScriptedProvider {
        async fn complete(
            &self,
            request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.prompts
                .lock()
                .unwrap()
                .push(request.messages[1].content.clone());
            Ok(CompletionResponse {
                content: self.response.clone(),
                model: "test-model".to_string(),
            })
        }
    }

    #[test]
    fn unknown_kinds_parse_as_other_and_missing_fields_default() {
        let parsed: EntryPointsResponse = serde_json::from_str(
            r#"{"entry_points":[{"file":"a","kind":"carrier_pigeon","name":"Send",
                "outputs":[{"kind":"smoke_signal"}]}]}"#,
        )
        .unwrap();
        let entry = &parsed.entry_points[0].entry;
        assert_eq!(entry.kind, EntryKind::Other);
        assert_eq!(entry.outputs[0].kind, OutputKind::Other);
        assert!(entry.verb.is_empty());
    }

    #[tokio::test]
    async fn reads_entry_points_once_and_reuses_them_while_files_are_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app/controllers")).unwrap();
        std::fs::write(
            dir.path().join("app/controllers/contracts_controller.rb"),
            "def sign; end",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("app/controllers/users_controller.rb"),
            "def show; end",
        )
        .unwrap();
        std::fs::write(dir.path().join("app/models/user.rb"), "class User; end").ok();
        let roles = RoleMap {
            roles: [
                (
                    "app/controllers/contracts_controller.rb",
                    FileRole::EntryPoint,
                ),
                ("app/controllers/users_controller.rb", FileRole::EntryPoint),
                ("app/models/user.rb", FileRole::Model),
            ]
            .into_iter()
            .map(|(p, r)| (PathBuf::from(p), r))
            .collect(),
        };
        let llm = ScriptedProvider {
            response: r#"{"entry_points":[
                {"file":"controllers/contracts_controller.rb","kind":"http_route",
                 "name":"POST /contracts/:id/sign","verb":"sign","resource":"contract",
                 "description":"A signatory signs a contract",
                 "outputs":[{"kind":"email","description":"confirmation to the parties"},
                            {"kind":"db_write","description":"contract marked signed"}]},
                {"file":"nowhere.rb","kind":"job","name":"Ghost"}]}"#
                .to_string(),
            prompts: Mutex::new(Vec::new()),
        };

        let inventory = build_entry_points(dir.path(), &roles, &llm).await.unwrap();
        let all: Vec<_> = inventory.iter().collect();
        assert_eq!(all.len(), 1);
        assert_eq!(
            all[0].0,
            Path::new("app/controllers/contracts_controller.rb")
        );
        assert_eq!(all[0].1.outputs.len(), 2);
        assert_eq!(inventory.distribution()[&EntryKind::HttpRoute], 1);
        // The model file is never sent.
        assert!(!llm.prompts.lock().unwrap()[0].contains("user.rb"));
        assert_eq!(llm.prompts.lock().unwrap().len(), 1);

        build_entry_points(dir.path(), &roles, &llm).await.unwrap();
        assert_eq!(llm.prompts.lock().unwrap().len(), 1);

        std::fs::write(
            dir.path().join("app/controllers/users_controller.rb"),
            "def show; x; end",
        )
        .unwrap();
        build_entry_points(dir.path(), &roles, &llm).await.unwrap();
        let prompts = llm.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(
            prompts[1].contains("users_controller") && !prompts[1].contains("contracts_controller")
        );
    }
}
