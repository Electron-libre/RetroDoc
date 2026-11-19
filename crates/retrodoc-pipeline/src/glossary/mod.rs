//! Models and glossary inventory (docs/findings.md §2, phase 7 step 2).
//!
//! The entities of the application (names, attributes, associations) are the
//! nouns of the business. They are read by the LLM from the files the
//! business-files pass located (`business_files`: models, but also plain
//! domain classes and services), or, when there is none, from the files the
//! role rules classified as [`FileRole::Model`], several small files per call
//! and cached by content hash. The vocabulary of the tests is a third source:
//! the descriptions of `describe`/`context`/`it`-style blocks, extracted
//! mechanically from the files classified as [`FileRole::Test`].
//!
//! The roles pass often finds no model file (a repository of plain domain
//! classes, no `models/` folder) or takes the migrations for models; that is
//! why the business-files pass exists (ADR 0025, which replaced the ADR 0018
//! fallback on `logic` and `entrypoint` files). With no business list and no
//! model file the glossary is empty, and `generate` says so.
//!
//! The result is `.retrodoc/cache/glossary.yaml`, which doubles as the cache:
//! a model file whose content hash is unchanged is not sent again. The
//! vocabulary is a hint to validate, not a truth (docs/findings.md §2 caveats).
//!
//! Not covered yet: the verbs (public methods, route actions), which belong
//! with the entry points inventory (step 3).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use retrodoc_ingest::signals::test_phrases;
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::artifact::Retention;
pub(crate) use retrodoc_core::artifacts::glossary::{
    Entity, Glossary, MergedEntity, ModelFile, TestVocabulary,
};

use crate::batched_read::{
    attribute, pending_chunks, reading_part, unread_kept, BatchedRead, PendingChunk,
};
use crate::brief::ProductBrief;
use crate::chunks::Splitter;
use crate::error::PipelineError;
use crate::request::Request;
use crate::roles::{FileRole, RoleMap};
use crate::text::read_file_lossy;

/// A longer model file is read in several chunks of about this size, so one
/// huge model can't crowd out the others in a batch.
const MAX_MODEL_FILE_CHARS: usize = 4_000;

pub(crate) const GLOSSARY_SYSTEM_PROMPT: &str =
    "You are extracting the business entities from the model \
files of a software application. For each entity (a business concept such as a contract, a \
company, an invoice; skip purely technical classes, concerns and mixins) give its name, one \
sentence on what it represents for the business, its main attributes (business meaning, not \
every column) and its associations to other entities. Reply with ONLY a single JSON object, no \
prose and no Markdown code fence, matching this shape: {\"entities\":[{\"file\":\"path as given\",\
\"name\":\"...\",\"description\":\"...\",\"attributes\":[\"...\"],\"associations\":[{\"kind\":\
\"has_many\",\"target\":\"...\"}]}]}. Use the file paths exactly as given.";

/// First line of the user prompt of the model files, before the files.
pub(crate) const GLOSSARY_HEADER: &str = "Model files:\n";

/// Same answer shape as [`GLOSSARY_SYSTEM_PROMPT`], for the files the
/// business-files pass located: some are plain domain classes, others
/// services, collections or helpers.
pub(crate) const BUSINESS_SYSTEM_PROMPT: &str =
    "You are looking for the business entities in the source \
files of a software application, located as holding its business. An entity is a plain class \
(or struct, or module) that holds business data and rules, such as a customer, an order or a \
contract; skip services, routers, collections of entities, helpers, mixins and anything \
purely technical. A file can hold no entity: then list none for it. For each entity give its \
name, one sentence on what it represents for the business, its main attributes (business \
meaning, not every field) and its associations to other entities. Reply with ONLY a single JSON \
object, no prose and no Markdown code fence, matching this shape: {\"entities\":[{\"file\":\
\"path as given\",\"name\":\"...\",\"description\":\"...\",\"attributes\":[\"...\"],\
\"associations\":[{\"kind\":\"has_many\",\"target\":\"...\"}]}]}. Use the file paths exactly \
as given.";

/// First line of the user prompt of the business files, before the files.
pub(crate) const BUSINESS_HEADER: &str = "Business files:\n";

/// The request of the model files.
pub(crate) static GLOSSARY_REQUEST: Request<GlossaryResponse> =
    Request::new(GLOSSARY_SYSTEM_PROMPT, &[GLOSSARY_HEADER]);

/// The request of the business files.
pub(crate) static BUSINESS_REQUEST: Request<GlossaryResponse> =
    Request::new(BUSINESS_SYSTEM_PROMPT, &[BUSINESS_HEADER]);

/// The reading of a file whose content hashes to `content_hash`: the entities of its chunks, merged.
pub(crate) fn read_model_file(content_hash: &str, entities: Vec<Entity>) -> ModelFile {
    ModelFile {
        content_hash: content_hash.to_string(),
        entities: merge_chunk_entities(entities),
    }
}

/// The entities of an answer by file (see [`attribute`]).
pub(crate) fn attribute_entities(
    response: GlossaryResponse,
    batch: &[PendingChunk],
) -> BTreeMap<String, Vec<Entity>> {
    attribute(
        response.entities,
        batch,
        "entity",
        |item| (item.file, item.entity),
        |entity| &entity.name,
    )
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct GlossaryResponse {
    #[serde(default)]
    pub(crate) entities: Vec<ResponseEntity>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ResponseEntity {
    #[serde(default)]
    file: String,
    #[serde(flatten)]
    entity: Entity,
}

/// Builds the glossary: reads the entities of the model files (only those
/// new or changed since the saved glossary go to the LLM, in batches) and
/// extracts the vocabulary of the test files. A batch whose answer is
/// unusable is skipped with a warning; its files are retried next run.
///
/// # Errors
///
/// Returns an error if a file can't be read, an LLM call fails (the batches
/// already read stay saved), or the glossary can't be saved.
pub async fn build_glossary(
    repo_root: &Path,
    roles: &RoleMap,
    business: &[PathBuf],
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
    keep: Retention,
) -> Result<Glossary, PipelineError> {
    let previous = Glossary::load(repo_root).unwrap_or_default();
    let mut glossary = Glossary {
        models: BTreeMap::new(),
        tests: previous.tests.clone(),
    };
    let to_read = files_to_read(roles, business);
    if keep == Retention::Others {
        let read: BTreeSet<&Path> = to_read.iter().copied().collect();
        glossary
            .models
            .extend(unread_kept(repo_root, &previous.models, &read));
    }
    let splitter = Splitter::new(&roles.chunk_boundaries);
    let reader = EntityReader {
        repo_root,
        brief,
        previous: &previous,
        splitter: &splitter,
        llm,
    };
    let (request, header, unit) = if business.is_empty() {
        (&GLOSSARY_REQUEST, GLOSSARY_HEADER, "model file(s)")
    } else {
        // Where the business lives, located by its own pass: services and
        // plain domain classes as much as models.
        (&BUSINESS_REQUEST, BUSINESS_HEADER, "business file(s)")
    };
    reader
        .read(
            to_read,
            request,
            &format!("{}{header}", brief.prompt_head()),
            unit,
            &mut glossary,
        )
        .await?;

    let tests = test_vocabulary(repo_root, roles)?;
    glossary.tests = tests;
    glossary.save(repo_root)?;
    Ok(glossary)
}

/// The files the glossary reads: the business files when there are some, else the files of the
/// `model` role.
fn files_to_read<'a>(roles: &'a RoleMap, business: &'a [PathBuf]) -> Vec<&'a Path> {
    if business.is_empty() {
        roles.files_with(FileRole::Model)
    } else {
        business.iter().map(PathBuf::as_path).collect()
    }
}

/// Reads the entities of a set of files: the ones whose content hash is
/// unchanged come from the previous glossary, the others go to the LLM.
struct EntityReader<'a> {
    repo_root: &'a Path,
    brief: &'a ProductBrief,
    previous: &'a Glossary,
    splitter: &'a Splitter,
    llm: &'a dyn LlmProvider,
}

impl EntityReader<'_> {
    async fn read(
        &self,
        paths: Vec<&Path>,
        request: &Request<GlossaryResponse>,
        header: &str,
        unit: &'static str,
        glossary: &mut Glossary,
    ) -> Result<(), PipelineError> {
        let pending = pending_chunks(
            self.repo_root,
            paths,
            reading_part(),
            self.brief,
            self.llm,
            |path, content| {
                self.splitter
                    .file_chunks(path, content, MAX_MODEL_FILE_CHARS, "glossary")
            },
            |path, hash| match self.previous.models.get(path) {
                Some(saved) if saved.content_hash == hash => {
                    glossary.models.insert(path.to_path_buf(), saved.clone());
                    true
                }
                _ => false,
            },
        )?;

        BatchedRead {
            pass: "glossary",
            unit,
            request,
            header,
            attribute: attribute_entities,
            finish: |glossary: &mut Glossary, path, hash, entities| {
                glossary
                    .models
                    .insert(path.to_path_buf(), read_model_file(hash, entities));
            },
            loud: &|_| false,
            checkpoint: &|glossary: &Glossary| Ok(glossary.save(self.repo_root)?),
        }
        .run(self.llm, &pending, glossary)
        .await
    }
}

/// The test phrases of every file classified [`FileRole::Test`] that has any.
fn test_vocabulary(
    repo_root: &Path,
    roles: &RoleMap,
) -> Result<Vec<TestVocabulary>, PipelineError> {
    let mut tests = Vec::new();
    for path in roles.files_with(FileRole::Test) {
        let phrases = test_phrases(&read_file_lossy(repo_root, path)?);
        if !phrases.is_empty() {
            tests.push(TestVocabulary {
                file: path.to_path_buf(),
                phrases,
            });
        }
    }
    Ok(tests)
}

/// Merges the entities seen in several chunks of one file: a class cut in two
/// is reported twice, its description kept from the first part and its
/// attributes and associations united.
pub(crate) fn merge_chunk_entities(entities: Vec<Entity>) -> Vec<Entity> {
    let mut merged: Vec<Entity> = Vec::new();
    for entity in entities {
        let Some(known) = merged.iter_mut().find(|e| e.name == entity.name) else {
            merged.push(entity);
            continue;
        };
        if known.description.is_empty() {
            known.description = entity.description;
        }
        for attribute in entity.attributes {
            if !known.attributes.contains(&attribute) {
                known.attributes.push(attribute);
            }
        }
        for association in entity.associations {
            if !known.associations.contains(&association) {
                known.associations.push(association);
            }
        }
    }
    merged
}

#[cfg(test)]
mod tests;
