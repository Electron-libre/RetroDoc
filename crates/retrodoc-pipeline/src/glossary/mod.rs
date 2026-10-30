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
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact, Retention};
use crate::batched_read::{
    attribute, pending_chunks, reading_instructions, unread_kept, BatchedRead, PendingChunk,
};
use crate::brief::ProductBrief;
use crate::chunks::Splitter;
use crate::error::PipelineError;
use crate::naming::normalize;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Association {
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Entity {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub attributes: Vec<String>,
    #[serde(default)]
    pub associations: Vec<Association>,
}

/// What was read from one model file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    pub content_hash: String,
    pub entities: Vec<Entity>,
}

impl ModelFile {
    /// The reading of a file whose content hashes to `content_hash`: the entities of its chunks,
    /// merged.
    pub(crate) fn read(content_hash: &str, entities: Vec<Entity>) -> Self {
        Self {
            content_hash: content_hash.to_string(),
            entities: merge_chunk_entities(entities),
        }
    }
}

/// Descriptions found in the tests of one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestVocabulary {
    pub file: PathBuf,
    pub phrases: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Glossary {
    #[serde(default)]
    pub models: BTreeMap<PathBuf, ModelFile>,
    #[serde(default)]
    pub tests: Vec<TestVocabulary>,
}

impl Glossary {
    /// Missing or unreadable: `None` (first run).
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Glossary.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::Glossary.path(repo_root), self)
    }

    /// Every entity with the file it was read from, in path order.
    pub fn entities(&self) -> impl Iterator<Item = (&Path, &Entity)> {
        self.models
            .iter()
            .flat_map(|(path, file)| file.entities.iter().map(move |e| (path.as_path(), e)))
    }

    /// The entities merged by name (case-insensitive): the LLM also lists the
    /// classes a file merely references, so one concept shows up under
    /// several files. Name and description come from its *home* entry (the
    /// one in a file named after it, `company.rb` for `Company`, else the
    /// richest one); attributes and associations are the union; `files` lists
    /// the home file first, then the others. Sorted by name.
    #[must_use]
    pub fn merged_entities(&self) -> Vec<MergedEntity> {
        let mut groups: BTreeMap<String, Vec<(&Path, &Entity)>> = BTreeMap::new();
        for (path, entity) in self.entities() {
            groups
                .entry(entity.name.trim().to_lowercase())
                .or_default()
                .push((path, entity));
        }
        groups
            .into_values()
            .map(|group| merge_group(&group))
            .collect()
    }

    #[must_use]
    pub fn phrase_count(&self) -> usize {
        self.tests.iter().map(|t| t.phrases.len()).sum()
    }
}

/// One business concept after merging the per-file entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedEntity {
    pub name: String,
    pub description: String,
    pub attributes: Vec<String>,
    pub associations: Vec<Association>,
    /// Files that mention the entity, the home file first.
    pub files: Vec<PathBuf>,
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

fn merge_group(group: &[(&Path, &Entity)]) -> MergedEntity {
    let richness = |entity: &Entity| {
        (
            entity.attributes.len() + entity.associations.len(),
            entity.description.len(),
        )
    };
    // `max_by_key` keeps the last maximum: iterate in reverse so that, on a
    // full tie, the first file in path order wins.
    let home = group
        .iter()
        .rev()
        .max_by_key(|(path, entity)| {
            let named_after = path
                .file_stem()
                .is_some_and(|stem| normalize(&stem.to_string_lossy()) == normalize(&entity.name));
            (named_after, richness(entity))
        })
        .expect("a group has at least one entry");

    let mut attributes: Vec<String> = Vec::new();
    let mut associations: Vec<Association> = Vec::new();
    let mut files: Vec<PathBuf> = vec![home.0.to_path_buf()];
    for (path, entity) in
        std::iter::once(home).chain(group.iter().filter(|e| !std::ptr::eq(*e, home)))
    {
        for attribute in &entity.attributes {
            if !attributes.iter().any(|a| a.eq_ignore_ascii_case(attribute)) {
                attributes.push(attribute.clone());
            }
        }
        for association in &entity.associations {
            if !associations.contains(association) {
                associations.push(association.clone());
            }
        }
        if !files.iter().any(|f| f == path) {
            files.push(path.to_path_buf());
        }
    }
    MergedEntity {
        name: home.1.name.trim().to_string(),
        description: home.1.description.clone(),
        attributes,
        associations,
        files,
    }
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
    let (prompt, header, unit) = if business.is_empty() {
        (GLOSSARY_SYSTEM_PROMPT, GLOSSARY_HEADER, "model file(s)")
    } else {
        // Where the business lives, located by its own pass: services and
        // plain domain classes as much as models.
        (BUSINESS_SYSTEM_PROMPT, BUSINESS_HEADER, "business file(s)")
    };
    reader
        .read(
            to_read,
            prompt,
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
        system_prompt: &'static str,
        header: &str,
        unit: &'static str,
        glossary: &mut Glossary,
    ) -> Result<(), PipelineError> {
        let pending = pending_chunks(
            self.repo_root,
            paths,
            &reading_instructions(),
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
            system_prompt,
            header,
            attribute: attribute_entities,
            finish: |glossary: &mut Glossary, path, hash, entities| {
                glossary
                    .models
                    .insert(path.to_path_buf(), ModelFile::read(hash, entities));
            },
            loud: &|_| false,
            checkpoint: &|glossary: &Glossary| glossary.save(self.repo_root),
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
