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

use crate::artifact::{load_yaml, save_yaml, Artifact, Retention};
use crate::batched_read::{attribute, pending_chunks, unread_kept, BatchedRead, PendingChunk};
use crate::brief::ProductBrief;
use crate::chunks::Splitter;
use crate::error::PipelineError;
use crate::roles::{FileRole, RoleMap};
use crate::slices::CodeIndex;

/// Entry point files carry many small actions: a bit more room than models.
/// A longer file is read in several chunks of about this size.
pub(crate) const MAX_ENTRY_FILE_CHARS: usize = 5_000;

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
email, file, event, external_call, db_write or other) and a short `description`. An entry point \
is what a user, a scheduler or another system triggers from outside the application. A function \
only called by other code of the application (a service, a query, a form, a serializer or a \
helper) is not one, even when it is public: skip it. A library is the exception: its public API \
is its entry points, so list the methods a user of the library calls. Reply with ONLY a single JSON object, no prose and no Markdown code fence, \
matching this shape: {\"entry_points\":[{\"file\":\"path as given\",\"kind\":\"http_route\",\
\"name\":\"...\",\"verb\":\"...\",\"resource\":\"...\",\"description\":\"...\",\"outputs\":\
[{\"kind\":\"email\",\"description\":\"...\"}]}]}. Use the file paths exactly as given.";

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Output {
    pub kind: OutputKind,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

impl EntryFile {
    /// The reading of a file whose content hashes to `content_hash`: the entry points of its chunks,
    /// the first of a name kept.
    pub(crate) fn read(content_hash: &str, mut entry_points: Vec<EntryPoint>) -> Self {
        let mut seen = BTreeSet::new();
        entry_points.retain(|e| seen.insert(e.name.clone()));
        Self {
            content_hash: content_hash.to_string(),
            entry_points,
        }
    }
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

/// As in the use cases pass: how far from an entry point's file the code it runs is followed.
const REACH_DEPTH: usize = 2;

impl EntryPoints {
    /// Whether the inventory is that of an application: a route, a job, a consumer or a webhook
    /// starts a behaviour. Else it is taken for a library, whose public API is its entry points
    /// (a command is not enough: a library often ships one, and a tool made of commands keeps
    /// all its entries, the safe side).
    #[must_use]
    pub fn is_application(&self) -> bool {
        self.iter().any(|(_, e)| {
            matches!(
                e.kind,
                EntryKind::HttpRoute | EntryKind::Job | EntryKind::Consumer | EntryKind::Webhook
            )
        })
    }

    /// The inventory of an application without the `public_api` entries that no route, command,
    /// job, consumer or webhook reaches: the public methods of its classes that nothing
    /// triggers are technical support, not behaviours. A file is reached when it holds such an
    /// entry point or is in its slice (`REACH_DEPTH` hops of references, see
    /// [`CodeIndex::slice`]). A library keeps all its entries. Returns it with the ids
    /// (`<file>#<name>`) of the entries left out; the saved inventory is not touched, so the
    /// choice is made again at each run, from the current code. Limits: the reach is by file and
    /// by the names the code mentions (a call through a callback or dynamic dispatch is missed),
    /// and a `generate` by contexts decides on the entry points read so far.
    #[must_use]
    pub fn without_unreached_public_api<'a>(
        mut self,
        repo_root: &Path,
        sources: impl IntoIterator<Item = &'a Path>,
    ) -> (Self, Vec<String>) {
        if !self.is_application() {
            return (self, Vec::new());
        }
        let starts: Vec<PathBuf> = self
            .files
            .iter()
            .filter(|(_, f)| f.entry_points.iter().any(|e| started_from_outside(e.kind)))
            .map(|(p, _)| p.clone())
            .collect();
        let index = CodeIndex::new(sources);
        let mut reached: BTreeSet<PathBuf> = starts.iter().cloned().collect();
        reached.extend(index.slice(repo_root, &starts, REACH_DEPTH, usize::MAX));
        let mut left_out = Vec::new();
        for (path, file) in &mut self.files {
            if reached.contains(path) {
                continue;
            }
            file.entry_points.retain(|e| {
                let keep = e.kind != EntryKind::PublicApi;
                if !keep {
                    left_out.push(format!("{}#{}", path.display(), e.name));
                }
                keep
            });
        }
        (self, left_out)
    }
}

/// Whether something other than a call to a public method (or a kind nobody could name) starts
/// the entry point.
fn started_from_outside(kind: EntryKind) -> bool {
    !matches!(kind, EntryKind::PublicApi | EntryKind::Other)
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct EntryPointsResponse {
    #[serde(default)]
    pub(crate) entry_points: Vec<ResponseEntry>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ResponseEntry {
    #[serde(default)]
    file: String,
    #[serde(flatten)]
    entry: EntryPoint,
}

/// Builds the entry points inventory from the files classified
/// [`FileRole::EntryPoint`] and the `business` files (where the business
/// lives, see `business_files`; a file in both is read once: the role alone
/// misses the public API of a library). Only files new or changed since the saved
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
    business: &[PathBuf],
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
    keep: Retention,
) -> Result<EntryPoints, PipelineError> {
    let previous = EntryPoints::load(repo_root).unwrap_or_default();
    let mut inventory = EntryPoints::default();
    let splitter = Splitter::new(&roles.chunk_boundaries);
    let to_read: BTreeSet<&Path> = roles
        .files_with(FileRole::EntryPoint)
        .into_iter()
        .chain(business.iter().map(PathBuf::as_path))
        .collect();
    if keep == Retention::Others {
        // Seeded first, so that the checkpoints and the final save hold them too.
        inventory
            .files
            .extend(unread_kept(repo_root, &previous.files, &to_read));
    }
    let pending = pending_chunks(
        repo_root,
        to_read,
        brief,
        llm,
        |path, content| splitter.file_chunks(path, content, MAX_ENTRY_FILE_CHARS, "entry points"),
        |path, hash| match previous.files.get(path) {
            Some(saved) if saved.content_hash == hash => {
                inventory.files.insert(path.to_path_buf(), saved.clone());
                true
            }
            _ => false,
        },
    )?;

    BatchedRead {
        pass: "entry points",
        unit: "file(s)",
        system_prompt: ENTRY_POINTS_SYSTEM_PROMPT,
        header: &format!("{}Files:\n", brief.prompt_head()),
        attribute: attribute_entry_points,
        finish: |inventory: &mut EntryPoints, path, hash, entry_points| {
            inventory
                .files
                .insert(path.to_path_buf(), EntryFile::read(hash, entry_points));
        },
        checkpoint: &|inventory: &EntryPoints| inventory.save(repo_root),
    }
    .run(llm, &pending, &mut inventory)
    .await?;

    inventory.save(repo_root)?;
    Ok(inventory)
}

/// The entry points of an answer, by the file of `group` each belongs to (see [`attribute`]).
pub(crate) fn attribute_entry_points(
    response: EntryPointsResponse,
    group: &[PendingChunk],
) -> BTreeMap<String, Vec<EntryPoint>> {
    attribute(
        response.entry_points,
        group,
        "entry point",
        |item| (item.file, item.entry),
        |entry| &entry.name,
    )
}

#[cfg(test)]
mod tests;
