//! One reading of the business files for the glossary and the entry points
//! (`issues/done/single_read_per_business_file.md`).
//!
//! Both passes read the business files `business_files` located, each with its own prompt. Here one call
//! per batch of chunks answers both: the entities and the entry points with their outputs. The answers
//! are written to the two artifacts (`glossary.yaml`, `entry-points.yaml`) with the same per-file hash the
//! passes compute, so [`crate::build_glossary`] and [`crate::build_entry_points`], run right after, find
//! those files up to date and read only the others (the entry point files outside the business list, or
//! a file this reading could not answer). Nothing downstream changes.
//!
//! The reading needs one model for both passes: with a model per pass (ADR 0023) that differs for the
//! glossary and the entry points, each pass reads on its own.

use std::collections::BTreeMap;
use std::path::PathBuf;

use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::batched_read::{BatchedRead, PendingChunk};
use crate::brief::ProductBrief;
use crate::chunks::Splitter;
use crate::entry_points::{
    attribute_entry_points, EntryFile, EntryPoint, EntryPoints, EntryPointsResponse, ResponseEntry,
    MAX_ENTRY_FILE_CHARS,
};
use crate::error::PipelineError;
use crate::fingerprints::hash_for_model;
use crate::glossary::{
    attribute_entities, merge_chunk_entities, Entity, Glossary, GlossaryResponse, ModelFile,
    ResponseEntity,
};
use crate::roles::RoleMap;
use crate::text::read_file_lossy;

// The wording of the two tasks is that of `glossary` (business prompt) and `entry_points`; keep them close.
const BUSINESS_READ_SYSTEM_PROMPT: &str = "You are reading the source files of a software \
application, located as holding its business, for two inventories at once. A long file is given in \
several parts (marked \"part i/n\"): report only what is visible in the part. \
(1) ENTRY POINTS: what a user, a scheduler or another system triggers from outside the application: \
HTTP routes and controller actions, CLI commands, background jobs, message consumers, webhooks, or, \
for a library, its public API. A function only called by other code of the application (a service, a \
query, a form, a serializer or a helper) is not one, even when it is public: skip it. A library is \
the exception: its public API is its entry points, so list the methods a user of the library calls. \
At most 30 per file. For each give: `kind` (http_route, cli_command, job, consumer, webhook, \
public_api or other); `name` as a reader would say it (e.g. \"POST /contracts/:id/sign\" or \
\"SendReminderJob\"); `verb`, the action in business words; `resource`, the business object it acts \
on; `description`, one sentence on what it does for the user or the business; and `outputs`, its \
observable effects, each with a `kind` (response, email, file, event, external_call, db_write or \
other) and a short `description`. The two inventories are independent: the same file can be both an \
entity and the source of entry points, so fill both lists for every file. \
(2) ENTITIES: a plain class (or struct, or module) that holds business data and rules, such as a \
customer, an order or a contract; skip services, routers, collections of entities, helpers, mixins \
and anything purely technical. A file can hold no entity: then list none for it. For each entity \
give its name, one sentence on what it represents for the business, its main attributes (business \
meaning, not every field) and its associations to other entities. \
A file can hold \
entities, entry points, both or neither. Reply with ONLY a single JSON object, no prose and no \
Markdown code fence, matching this shape: {\"entry_points\":[{\"file\":\"path as given\",\
\"kind\":\"http_route\",\"name\":\"...\",\"verb\":\"...\",\"resource\":\"...\",\
\"description\":\"...\",\"outputs\":[{\"kind\":\"email\",\"description\":\"...\"}]}],\
\"entities\":[{\"file\":\"path as given\",\"name\":\"...\",\"description\":\"...\",\
\"attributes\":[\"...\"],\"associations\":[{\"kind\":\"has_many\",\"target\":\"...\"}]}]}. Use the file paths exactly as given.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BusinessReadResponse {
    #[serde(default)]
    entry_points: Vec<ResponseEntry>,
    #[serde(default)]
    entities: Vec<ResponseEntity>,
}

/// What one answer found in a file.
enum Found {
    Entity(Entity),
    Entry(EntryPoint),
}

/// The two artifacts, filled together.
struct Both {
    glossary: Glossary,
    entry_points: EntryPoints,
}

fn attribute(
    response: BusinessReadResponse,
    group: &[PendingChunk],
) -> BTreeMap<String, Vec<Found>> {
    let mut found: BTreeMap<String, Vec<Found>> = BTreeMap::new();
    let entities = attribute_entities(
        GlossaryResponse {
            entities: response.entities,
        },
        group,
    );
    for (file, items) in entities {
        found
            .entry(file)
            .or_default()
            .extend(items.into_iter().map(Found::Entity));
    }
    let entries = attribute_entry_points(
        EntryPointsResponse {
            entry_points: response.entry_points,
        },
        group,
    );
    for (file, items) in entries {
        found
            .entry(file)
            .or_default()
            .extend(items.into_iter().map(Found::Entry));
    }
    found
}

/// Reads the `business` files that are new or changed since the saved artifacts, once for both passes,
/// and writes `glossary.yaml` and `entry-points.yaml`. Does nothing without business files, or when the
/// two passes use different models. A file whose answer is unusable is left to the passes.
///
/// # Errors
///
/// Returns an error if a file can't be read, an LLM call fails (the batches already read stay saved), or
/// an artifact can't be saved.
pub async fn read_business_files(
    repo_root: &std::path::Path,
    roles: &RoleMap,
    business: &[PathBuf],
    brief: &ProductBrief,
    glossary_llm: &dyn LlmProvider,
    entry_points_llm: &dyn LlmProvider,
) -> Result<(), PipelineError> {
    if business.is_empty() || glossary_llm.model() != entry_points_llm.model() {
        return Ok(());
    }
    let mut state = Both {
        glossary: Glossary::load(repo_root).unwrap_or_default(),
        entry_points: EntryPoints::load(repo_root).unwrap_or_default(),
    };
    let splitter = Splitter::new(&roles.chunk_boundaries);
    // One item per chunk of a changed file: (path, file hash, chunk text).
    let mut pending: Vec<PendingChunk> = Vec::new();
    for path in business {
        let content = read_file_lossy(repo_root, path)?;
        let hash = hash_for_model(&brief.hash_with(&content), glossary_llm);
        let current = state
            .glossary
            .models
            .get(path)
            .is_some_and(|saved| saved.content_hash == hash)
            && state
                .entry_points
                .files
                .get(path)
                .is_some_and(|saved| saved.content_hash == hash);
        if current {
            continue;
        }
        for chunk in splitter.file_chunks(path, &content, MAX_ENTRY_FILE_CHARS, "business files") {
            pending.push((path.clone(), hash.clone(), chunk));
        }
    }

    BatchedRead {
        pass: "business files",
        unit: "file(s)",
        system_prompt: BUSINESS_READ_SYSTEM_PROMPT,
        header: &format!("{}Business files:\n", brief.prompt_head()),
        attribute,
        finish: |state: &mut Both, path, hash, items| {
            let (mut entities, mut entry_points) = (Vec::new(), Vec::new());
            for item in items {
                match item {
                    Found::Entity(entity) => entities.push(entity),
                    Found::Entry(entry) => entry_points.push(entry),
                }
            }
            let mut seen = std::collections::BTreeSet::new();
            entry_points.retain(|e| seen.insert(e.name.clone()));
            state.glossary.models.insert(
                path.to_path_buf(),
                ModelFile {
                    content_hash: hash.to_string(),
                    entities: merge_chunk_entities(entities),
                },
            );
            state.entry_points.files.insert(
                path.to_path_buf(),
                EntryFile {
                    content_hash: hash.to_string(),
                    entry_points,
                },
            );
        },
        checkpoint: &|state: &Both| {
            state.glossary.save(repo_root)?;
            state.entry_points.save(repo_root)
        },
    }
    .run(glossary_llm, &pending, &mut state)
    .await
}

#[cfg(test)]
mod tests;
