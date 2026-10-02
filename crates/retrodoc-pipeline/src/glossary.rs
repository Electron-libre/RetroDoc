//! Models and glossary inventory (PLAN.md §7.1, phase 7 step 2).
//!
//! The entities of the application (names, attributes, associations) are the
//! nouns of the business. They are read by the LLM from the files the role
//! rules classified as [`FileRole::Model`] only, several small files per call
//! and cached by content hash. The vocabulary of the tests is a third source:
//! the descriptions of `describe`/`context`/`it`-style blocks, extracted
//! mechanically from the files classified as [`FileRole::Test`].
//!
//! The result is `.retrodoc/cache/glossary.yaml`, which doubles as the cache:
//! a model file whose content hash is unchanged is not sent again. The
//! vocabulary is a hint to validate, not a truth (PLAN.md §7.1 caveats).
//!
//! Not covered yet: the verbs (public methods, route actions), which belong
//! with the entry points inventory (step 3).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::progress::Progress;
use crate::repo_map::{read_file_lossy, truncate_chars};
use crate::response::complete_json;
use crate::roles::{FileRole, RoleMap};
use crate::use_cases::resolve_cited_path;

const GLOSSARY_RELATIVE_PATH: &str = ".retrodoc/cache/glossary.yaml";

/// Per-file cut, so one huge model can't crowd out the others in a batch.
const MAX_MODEL_FILE_CHARS: usize = 4_000;
/// Characters of model code per LLM call.
const BATCH_CHARS: usize = 12_000;
const MAX_PHRASES_PER_TEST_FILE: usize = 30;
const MAX_PHRASE_CHARS: usize = 160;

const GLOSSARY_SYSTEM_PROMPT: &str = "You are extracting the business entities from the model \
files of a software application. For each entity (a business concept such as a contract, a \
company, an invoice; skip purely technical classes, concerns and mixins) give its name, one \
sentence on what it represents for the business, its main attributes (business meaning, not \
every column) and its associations to other entities. Reply with ONLY a single JSON object, no \
prose and no Markdown code fence, matching this shape: {\"entities\":[{\"file\":\"path as given\",\
\"name\":\"...\",\"description\":\"...\",\"attributes\":[\"...\"],\"associations\":[{\"kind\":\
\"has_many\",\"target\":\"...\"}]}]}. Use the file paths exactly as given.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Association {
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        let raw = std::fs::read_to_string(repo_root.join(GLOSSARY_RELATIVE_PATH)).ok()?;
        serde_yaml::from_str(&raw).ok()
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        let path = repo_root.join(GLOSSARY_RELATIVE_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PipelineError::ArtifactIo {
                path: path.clone(),
                source,
            })?;
        }
        let raw = serde_yaml::to_string(self)?;
        std::fs::write(&path, raw).map_err(|source| PipelineError::ArtifactIo { path, source })
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

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
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

#[derive(Debug, Deserialize)]
struct GlossaryResponse {
    #[serde(default)]
    entities: Vec<ResponseEntity>,
}

#[derive(Debug, Deserialize)]
struct ResponseEntity {
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
    llm: &dyn LlmProvider,
) -> Result<Glossary, PipelineError> {
    let previous = Glossary::load(repo_root).unwrap_or_default();
    let mut glossary = Glossary {
        models: BTreeMap::new(),
        tests: previous.tests.clone(),
    };
    let mut pending: Vec<(PathBuf, String, String)> = Vec::new();

    for path in roles.files_with(FileRole::Model) {
        let content = read_file_lossy(repo_root, path)?;
        let hash = hash_content(&content);
        match previous.models.get(path) {
            Some(saved) if saved.content_hash == hash => {
                glossary.models.insert(path.to_path_buf(), saved.clone());
            }
            _ => pending.push((
                path.to_path_buf(),
                hash,
                truncate_chars(&content, MAX_MODEL_FILE_CHARS),
            )),
        }
    }

    let batches = batches(&pending);
    let mut progress = Progress::new("glossary", batches.len());
    for batch in batches {
        progress.begin(&format!(
            "{} model file(s), from {}",
            batch.len(),
            batch[0].0.display()
        ));
        let mut prompt = String::from("Model files:\n");
        for (path, _, content) in batch {
            let _ = write!(prompt, "\n--- {} ---\n{content}\n", path.display());
        }
        let Some(response) =
            complete_json::<GlossaryResponse>(llm, GLOSSARY_SYSTEM_PROMPT, &prompt, "glossary")
                .await?
        else {
            continue;
        };

        let allowed: BTreeSet<String> = batch
            .iter()
            .map(|(path, _, _)| path.to_string_lossy().into_owned())
            .collect();
        let mut found: BTreeMap<String, Vec<Entity>> = BTreeMap::new();
        for item in response.entities {
            let target = if batch.len() == 1 {
                allowed.iter().next().cloned()
            } else {
                resolve_cited_path(&item.file, &allowed)
            };
            match target {
                Some(file) if !item.entity.name.trim().is_empty() => {
                    found.entry(file).or_default().push(item.entity);
                }
                Some(_) => {}
                None => tracing::warn!(
                    file = %item.file,
                    entity = %item.entity.name,
                    "entity attributed to an unknown file, dropped"
                ),
            }
        }
        for (path, hash, _) in batch {
            let entities = found
                .remove(path.to_string_lossy().as_ref())
                .unwrap_or_default();
            glossary.models.insert(
                path.clone(),
                ModelFile {
                    content_hash: hash.clone(),
                    entities,
                },
            );
        }
        // Saved after every batch: a failure later in a long run (a call
        // timing out) must not lose the batches already read.
        glossary.save(repo_root)?;
    }

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

    glossary.tests = tests;
    glossary.save(repo_root)?;
    Ok(glossary)
}

/// Groups files so that each batch holds about [`BATCH_CHARS`] of code
/// (at least one file).
pub(crate) fn batches(files: &[(PathBuf, String, String)]) -> Vec<&[(PathBuf, String, String)]> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut size = 0;
    for (i, (_, _, content)) in files.iter().enumerate() {
        let len = content.chars().count();
        if i > start && size + len > BATCH_CHARS {
            out.push(&files[start..i]);
            start = i;
            size = 0;
        }
        size += len;
    }
    if start < files.len() {
        out.push(&files[start..]);
    }
    out
}

/// Block keywords whose first string argument describes a behaviour, across
/// the `RSpec` / Jest / Mocha / Cucumber-like families.
const TEST_KEYWORDS: &[&str] = &[
    "describe", "context", "it", "scenario", "feature", "specify", "test", "example",
];

/// Descriptions of the test blocks of a file (`it "signs a contract" do`,
/// `test('rejects an expired token', …)`) and the words of `def test_foo_bar`
/// style names, deduplicated, in file order.
fn test_phrases(content: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut phrases = Vec::new();
    for line in content.lines() {
        if phrases.len() >= MAX_PHRASES_PER_TEST_FILE {
            break;
        }
        let line = line.trim();
        let phrase = if let Some(name) = line.strip_prefix("def test_") {
            let name: String = name
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            Some(name.replace('_', " "))
        } else {
            TEST_KEYWORDS
                .iter()
                .find_map(|keyword| quoted_argument(line, keyword))
        };
        if let Some(phrase) = phrase {
            let phrase: String = phrase.trim().chars().take(MAX_PHRASE_CHARS).collect();
            if !phrase.is_empty() && seen.insert(phrase.clone()) {
                phrases.push(phrase);
            }
        }
    }
    phrases
}

/// The quoted string right after `keyword` (`it "x"`, `it("x"`, `it 'x'`).
fn quoted_argument(line: &str, keyword: &str) -> Option<String> {
    let rest = line.strip_prefix(keyword)?;
    let rest = rest.strip_prefix('(').unwrap_or(rest).trim_start();
    // A keyword followed directly by a letter is another word (`items`).
    if rest.len() == line.len() - keyword.len() && !line[keyword.len()..].starts_with(' ') {
        return None;
    }
    let quote = rest
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'))?;
    let body = &rest[1..];
    body.find(quote).map(|end| body[..end].to_string())
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

    fn roles(entries: &[(&str, FileRole)]) -> RoleMap {
        RoleMap {
            roles: entries
                .iter()
                .map(|(p, r)| (PathBuf::from(p), *r))
                .collect(),
        }
    }

    #[test]
    fn extracts_test_descriptions() {
        let content = r##"
RSpec.describe Contract do
  describe "#sign" do
    context 'when the signatory is a partner' do
      it "marks the contract as signed" do
  items.each { }
  it("rejects an expired token", () => {})
  it "marks the contract as signed" do
  def test_cancel_subscription_twice
"##;
        assert_eq!(
            test_phrases(content),
            vec![
                "#sign",
                "when the signatory is a partner",
                "marks the contract as signed",
                "rejects an expired token",
                "cancel subscription twice",
            ]
        );
    }

    #[test]
    fn merges_entities_listed_under_several_files() {
        let entity =
            |name: &str, description: &str, attributes: &[&str], target: Option<&str>| Entity {
                name: name.to_string(),
                description: description.to_string(),
                attributes: attributes.iter().map(|a| (*a).to_string()).collect(),
                associations: target
                    .map(|t| Association {
                        kind: "has_many".to_string(),
                        target: t.to_string(),
                    })
                    .into_iter()
                    .collect(),
            };
        let file = |entities| ModelFile {
            content_hash: String::new(),
            entities,
        };
        let glossary = Glossary {
            models: BTreeMap::from([
                (
                    PathBuf::from("app/models/actions/company_user_actions.rb"),
                    file(vec![entity(
                        "Company",
                        "short",
                        &["name"],
                        Some("CompanyUser"),
                    )]),
                ),
                (
                    PathBuf::from("app/models/company.rb"),
                    file(vec![entity(
                        "company",
                        "A business organization",
                        &["Name", "siret"],
                        Some("Worksite"),
                    )]),
                ),
                (
                    PathBuf::from("app/models/user.rb"),
                    file(vec![entity("User", "A person", &[], None)]),
                ),
            ]),
            tests: Vec::new(),
        };

        let merged = glossary.merged_entities();
        assert_eq!(merged.len(), 2);
        let company = &merged[0];
        assert_eq!(company.name, "company");
        assert_eq!(company.description, "A business organization");
        assert_eq!(company.attributes, vec!["Name", "siret"]);
        assert_eq!(company.associations.len(), 2);
        assert_eq!(
            company.files,
            vec![
                PathBuf::from("app/models/company.rb"),
                PathBuf::from("app/models/actions/company_user_actions.rb"),
            ]
        );
        assert_eq!(merged[1].name, "User");
    }

    #[test]
    fn batches_respect_the_char_budget() {
        let file = |n: &str, len: usize| (PathBuf::from(n), String::new(), "x".repeat(len));
        let files = vec![
            file("a", 7_000),
            file("b", 7_000),
            file("c", 100),
            file("d", 20_000),
        ];
        let sizes: Vec<usize> = batches(&files).iter().map(|b| b.len()).collect();
        assert_eq!(sizes, vec![1, 2, 1]);
    }

    #[tokio::test]
    async fn reads_entities_once_and_reuses_them_while_files_are_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app/models")).unwrap();
        std::fs::create_dir_all(dir.path().join("spec")).unwrap();
        std::fs::write(
            dir.path().join("app/models/contract.rb"),
            "class Contract; end",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("app/models/company.rb"),
            "class Company; end",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("spec/contract_spec.rb"),
            "it \"is signed\" do",
        )
        .unwrap();
        let roles = roles(&[
            ("app/models/contract.rb", FileRole::Model),
            ("app/models/company.rb", FileRole::Model),
            ("spec/contract_spec.rb", FileRole::Test),
        ]);
        let llm = ScriptedProvider {
            response: r#"{"entities":[
                {"file":"models/contract.rb","name":"Contract","description":"An agreement",
                 "attributes":["signed_at"],"associations":[{"kind":"belongs_to","target":"Company"}]},
                {"file":"app/models/company.rb","name":"Company"},
                {"file":"nowhere.rb","name":"Ghost"}]}"#
                .to_string(),
            prompts: Mutex::new(Vec::new()),
        };

        let glossary = build_glossary(dir.path(), &roles, &llm).await.unwrap();
        let entities: Vec<_> = glossary
            .entities()
            .map(|(p, e)| (p.to_string_lossy().into_owned(), e.name.clone()))
            .collect();
        assert_eq!(
            entities,
            vec![
                ("app/models/company.rb".to_string(), "Company".to_string()),
                ("app/models/contract.rb".to_string(), "Contract".to_string()),
            ]
        );
        assert_eq!(glossary.phrase_count(), 1);
        assert_eq!(llm.prompts.lock().unwrap().len(), 1);

        // Second run: nothing changed, no call.
        build_glossary(dir.path(), &roles, &llm).await.unwrap();
        assert_eq!(llm.prompts.lock().unwrap().len(), 1);

        // A changed file is the only one sent again.
        std::fs::write(
            dir.path().join("app/models/company.rb"),
            "class Company; x; end",
        )
        .unwrap();
        build_glossary(dir.path(), &roles, &llm).await.unwrap();
        let prompts = llm.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[1].contains("company.rb") && !prompts[1].contains("contract.rb"));
    }
}
