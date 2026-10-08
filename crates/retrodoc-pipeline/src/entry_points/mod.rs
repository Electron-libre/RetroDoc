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
use std::path::{Path, PathBuf};

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::batched_read::{group_paths, BatchedRead, PendingChunk};
use crate::brief::ProductBrief;
use crate::chunks::{strip_part_marker, Splitter};
use crate::error::PipelineError;
use crate::repo_map::read_file_lossy;
use crate::roles::{FileRole, RoleMap};
use crate::use_cases::resolve_cited_path;

/// Entry point files carry many small actions: a bit more room than models.
/// A longer file is read in several chunks of about this size.
const MAX_ENTRY_FILE_CHARS: usize = 5_000;

const ENTRY_POINTS_SYSTEM_PROMPT: &str = "You are inventorying the entry points of a software \
application from the files that define them: HTTP routes and controller actions, CLI commands, \
background jobs and schedulers, message consumers, webhooks, or, for a library, its public API. \
List the entry points defined in each file. A long file is given in several parts (marked \"part i/n\"): list only the entry points \
visible in the part. For a routing file (route declarations) give one \
entry per resource or namespace, with the main actions as the verb (e.g. \"list, show, create\"), \
not one per route: the controllers list the individual actions. At most 30 entry points per file. \
For each give: `kind` (http_route, cli_command, job, consumer, webhook, \
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
        load_yaml(&Artifact::EntryPoints.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::EntryPoints.path(repo_root), self)
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
/// Returns an error if a file can't be read, an LLM call fails (the batches
/// already read stay saved), or the inventory can't be saved.
pub async fn build_entry_points(
    repo_root: &Path,
    roles: &RoleMap,
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
) -> Result<EntryPoints, PipelineError> {
    let previous = EntryPoints::load(repo_root).unwrap_or_default();
    let mut inventory = EntryPoints::default();
    let splitter = Splitter::new(&roles.chunk_boundaries);
    // One item per chunk of a changed file: (path, file hash, chunk text).
    let mut pending: Vec<PendingChunk> = Vec::new();
    for path in roles.files_with(FileRole::EntryPoint) {
        let content = read_file_lossy(repo_root, path)?;
        let hash = brief.hash_with(&content);
        match previous.files.get(path) {
            Some(saved) if saved.content_hash == hash => {
                inventory.files.insert(path.to_path_buf(), saved.clone());
            }
            _ => {
                let chunks =
                    splitter.file_chunks(path, &content, MAX_ENTRY_FILE_CHARS, "entry points");
                for chunk in chunks {
                    pending.push((path.to_path_buf(), hash.clone(), chunk));
                }
            }
        }
    }

    BatchedRead {
        pass: "entry points",
        unit: "file(s)",
        system_prompt: ENTRY_POINTS_SYSTEM_PROMPT,
        header: &format!("{}Files:\n", brief.prompt_head()),
        attribute: attribute_entry_points,
        finish: |inventory: &mut EntryPoints, path, hash, mut entry_points| {
            let mut seen = BTreeSet::new();
            entry_points.retain(|e| seen.insert(e.name.clone()));
            inventory.files.insert(
                path.to_path_buf(),
                EntryFile {
                    content_hash: hash.to_string(),
                    entry_points,
                },
            );
        },
        checkpoint: &|inventory: &EntryPoints| inventory.save(repo_root),
    }
    .run(llm, &pending, &mut inventory)
    .await?;

    inventory.save(repo_root)?;
    Ok(inventory)
}

/// The entry points of an answer, by the file of `group` each belongs to (an
/// entry point of an unknown file is dropped, with a warning).
fn attribute_entry_points(
    response: EntryPointsResponse,
    group: &[PendingChunk],
) -> BTreeMap<String, Vec<EntryPoint>> {
    let allowed = group_paths(group);
    let mut found: BTreeMap<String, Vec<EntryPoint>> = BTreeMap::new();
    for item in response.entry_points {
        let target = if allowed.len() == 1 {
            allowed.iter().next().cloned()
        } else {
            resolve_cited_path(strip_part_marker(&item.file), &allowed)
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
    found
}

#[cfg(test)]
mod tests;
