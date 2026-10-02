//! Business actors (PLAN.md §7.1, phase 7 step 4c): who uses the application,
//! in business terms ("contract manager", "signatory", "e-signature
//! provider") rather than "Developer" or "System". They are derived from the
//! authorization code (abilities, policies, roles, permissions: the files
//! whose name says so) and from the user-like entities of the glossary, in
//! one LLM call, saved as `.retrodoc/cache/actors.yaml`. The use cases pass
//! then names its actors from this list.
//!
//! The list is reused while its inputs (the content of those files and the
//! user-like entities) are unchanged.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_core::model::ActorKind;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::fingerprints::fingerprint;
use crate::repo_map::{read_file_lossy, truncate_chars};
use crate::response::complete_json;
use crate::surface::Surface;

const ACTORS_RELATIVE_PATH: &str = ".retrodoc/cache/actors.yaml";

const MAX_AUTH_FILES: usize = 12;
const MAX_AUTH_FILE_CHARS: usize = 3_000;
const MAX_USER_ENTITIES: usize = 15;
const MAX_ATTRIBUTES_SHOWN: usize = 8;

/// File name words that mark authorization code, most telling first.
const AUTH_WORDS: &[&str] = &[
    "ability",
    "abilities",
    "policy",
    "policies",
    "permission",
    "permissions",
    "role",
    "roles",
    "authorization",
    "authorize",
    "acl",
    "authentication",
    "authenticate",
    "auth",
    "guard",
];

/// Entity name words that mark someone who acts in the application.
const USER_WORDS: &[&str] = &[
    "user",
    "role",
    "profile",
    "account",
    "member",
    "admin",
    "customer",
    "client",
    "partner",
    "signatory",
    "operator",
    "employee",
    "technician",
];

const ACTORS_SYSTEM_PROMPT: &str = "You are identifying the actors of a software application, in \
business terms. From its authorization code (abilities, policies, roles, permissions) and its \
user-related entities, list who uses it or acts on it: the roles a person can have (e.g. \"Contract \
manager\", \"Signatory\", \"Subcontractor administrator\"), and the external systems that call it \
or that it depends on to act (e.g. \"E-signature provider\"). Name each actor as the business \
would, not as the code does (not \"User\", \"Admin class\" or \"System\"). For each give a `name`, \
a `kind` (\"human\" or \"system\"), a one-sentence `description` of what it does in the \
application, and `evidence`, the file paths it is derived from. Reply with ONLY a single JSON \
object, no prose and no Markdown code fence, matching this shape: {\"actors\":[{\"name\":\"...\",\
\"kind\":\"human\",\"description\":\"...\",\"evidence\":[\"...\"]}]}.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusinessActor {
    pub name: String,
    pub kind: ActorKind,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Actors {
    /// Hash of what the list was derived from; the list is reused while it
    /// is unchanged.
    #[serde(default)]
    pub input_hash: String,
    #[serde(default)]
    pub actors: Vec<BusinessActor>,
}

impl Actors {
    /// Missing or unreadable: `None` (first run).
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(repo_root.join(ACTORS_RELATIVE_PATH)).ok()?;
        serde_yaml::from_str(&raw).ok()
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        let path = repo_root.join(ACTORS_RELATIVE_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PipelineError::ArtifactIo {
                path: path.clone(),
                source,
            })?;
        }
        let raw = serde_yaml::to_string(self)?;
        std::fs::write(&path, raw).map_err(|source| PipelineError::ArtifactIo { path, source })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    /// The known actor `name` designates (case-insensitive), as spelled in
    /// the list.
    #[must_use]
    pub fn canonical(&self, name: &str) -> Option<&BusinessActor> {
        let name = name.trim();
        self.actors
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    }

    /// The list as a prompt section.
    #[must_use]
    pub fn prompt_section(&self) -> String {
        let mut out = String::new();
        for actor in &self.actors {
            let kind = if actor.kind == ActorKind::Human {
                "human"
            } else {
                "system"
            };
            let _ = writeln!(out, "- {} ({kind}): {}", actor.name, actor.description);
        }
        out
    }

    /// Strings to hash: a changed list must invalidate the use cases.
    pub fn fingerprint_parts(&self) -> impl Iterator<Item = String> + '_ {
        self.actors
            .iter()
            .map(|a| format!("actor {} {:?} {}", a.name, a.kind, a.description))
    }
}

#[derive(Debug, Deserialize)]
struct RawActors {
    #[serde(default)]
    actors: Vec<RawActor>,
}

#[derive(Debug, Deserialize)]
struct RawActor {
    name: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    evidence: Vec<String>,
}

/// Words of an identifier, split on non-alphanumerics and on lower→upper
/// case changes, lowercased.
fn words(identifier: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for c in identifier.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous_lower = false;
            continue;
        }
        if c.is_uppercase() && previous_lower && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        previous_lower = c.is_lowercase() || c.is_numeric();
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// The files that look like authorization code, by the words of their name:
/// the ones with the most telling word first, then in path order, at most
/// [`MAX_AUTH_FILES`].
#[must_use]
pub fn authorization_files(source_files: &[PathBuf]) -> Vec<PathBuf> {
    let mut found: Vec<(usize, &PathBuf)> = source_files
        .iter()
        .filter_map(|path| {
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let stem_words = words(&stem);
            AUTH_WORDS
                .iter()
                .position(|w| stem_words.iter().any(|s| s == w))
                .map(|rank| (rank, path))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    found
        .into_iter()
        .take(MAX_AUTH_FILES)
        .map(|(_, path)| path.clone())
        .collect()
}

/// Entities that look like someone acting in the application, as prompt
/// lines with their main attributes (a `role` or `admin` column is telling).
fn user_entity_lines(surface: &Surface) -> Vec<String> {
    surface
        .entities
        .iter()
        .filter(|e| {
            words(&e.name)
                .iter()
                .any(|w| USER_WORDS.contains(&w.as_str()))
                || e.attributes.iter().any(|a| {
                    a.to_lowercase().contains("role") || a.to_lowercase().contains("admin")
                })
        })
        .take(MAX_USER_ENTITIES)
        .map(|e| {
            let attributes: Vec<&str> = e
                .attributes
                .iter()
                .take(MAX_ATTRIBUTES_SHOWN)
                .map(String::as_str)
                .collect();
            format!(
                "- {}: {} [{}]",
                e.name,
                e.description,
                attributes.join(", ")
            )
        })
        .collect()
}

/// Identifies the business actors of the application. Reuses the saved list
/// when the authorization files and the user-like entities are unchanged
/// (and `force` is false); an unusable answer yields an empty list, not
/// saved, with a warning.
///
/// # Errors
///
/// Returns an error if the LLM call fails or the list can't be saved.
pub async fn build_actors(
    repo_root: &Path,
    source_files: &[PathBuf],
    surface: &Surface,
    llm: &dyn LlmProvider,
    force: bool,
) -> Result<Actors, PipelineError> {
    let files = authorization_files(source_files);
    let mut contents: Vec<(PathBuf, String)> = Vec::new();
    for path in files {
        match read_file_lossy(repo_root, &path) {
            Ok(content) => contents.push((path, content)),
            Err(err) => {
                tracing::warn!(path = %path.display(), error = %err, "authorization file skipped");
            }
        }
    }
    let entity_lines = user_entity_lines(surface);
    if contents.is_empty() && entity_lines.is_empty() {
        tracing::warn!("no authorization code nor user-like entity found, no actors identified");
        return Ok(Actors::default());
    }

    let input_hash = fingerprint(
        contents
            .iter()
            .map(|(path, content)| format!("{}\n{}", path.display(), hash_content(content)))
            .chain(entity_lines.iter().cloned()),
    );
    if !force {
        if let Some(saved) = Actors::load(repo_root).filter(|a| a.input_hash == input_hash) {
            tracing::info!("actors unchanged, reused");
            return Ok(saved);
        }
    }

    let mut prompt = String::new();
    if !entity_lines.is_empty() {
        prompt.push_str("User-related entities:\n");
        for line in &entity_lines {
            let _ = writeln!(prompt, "{line}");
        }
    }
    for (path, content) in &contents {
        let _ = write!(
            prompt,
            "\n--- {} ---\n{}\n",
            path.display(),
            truncate_chars(content, MAX_AUTH_FILE_CHARS)
        );
    }

    let Some(raw) =
        complete_json::<RawActors>(llm, ACTORS_SYSTEM_PROMPT, &prompt, "business actors").await?
    else {
        return Ok(Actors::default());
    };

    let mut seen = BTreeSet::new();
    let actors = raw
        .actors
        .into_iter()
        .filter(|a| !a.name.trim().is_empty() && seen.insert(a.name.trim().to_lowercase()))
        .map(|a| BusinessActor {
            name: a.name.trim().to_string(),
            kind: if a.kind.eq_ignore_ascii_case("system") {
                ActorKind::System
            } else {
                ActorKind::Human
            },
            description: a.description,
            evidence: a.evidence,
        })
        .collect();
    let actors = Actors { input_hash, actors };
    actors.save(repo_root)?;
    Ok(actors)
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
                model: "m".to_string(),
            })
        }
    }

    fn paths(list: &[&str]) -> Vec<PathBuf> {
        list.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn splits_identifiers_into_words() {
        assert_eq!(words("ContractPolicy"), vec!["contract", "policy"]);
        assert_eq!(words("user_roles"), vec!["user", "roles"]);
        assert_eq!(words("ability"), vec!["ability"]);
    }

    #[test]
    fn finds_authorization_files_by_name_most_telling_first() {
        let files = paths(&[
            "app/models/ability.rb",
            "app/policies/contract_policy.rb",
            "app/models/author.rb",
            "app/services/role_checker.rb",
            "app/controllers/authentication_controller.rb",
            "app/models/roster.rb",
        ]);
        assert_eq!(
            authorization_files(&files),
            paths(&[
                "app/models/ability.rb",
                "app/policies/contract_policy.rb",
                "app/services/role_checker.rb",
                "app/controllers/authentication_controller.rb",
            ])
        );
    }

    #[tokio::test]
    async fn identifies_actors_once_and_reuses_them_while_the_inputs_are_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();
        std::fs::write(
            dir.path().join("app/ability.rb"),
            "can :sign, Contract if manager?",
        )
        .unwrap();
        let files = paths(&["app/ability.rb", "app/other.rb"]);
        let llm = ScriptedProvider {
            response: r#"{"actors":[
                {"name":"Contract manager","kind":"human","description":"Manages contracts","evidence":["app/ability.rb"]},
                {"name":"contract MANAGER","kind":"human"},
                {"name":"E-signature provider","kind":"system","description":"Signs"},
                {"name":" ","kind":"human"}]}"#
                .to_string(),
            prompts: Mutex::new(Vec::new()),
        };

        let actors = build_actors(dir.path(), &files, &Surface::default(), &llm, false)
            .await
            .unwrap();
        assert_eq!(actors.actors.len(), 2);
        assert_eq!(
            actors.canonical(" contract manager ").unwrap().name,
            "Contract manager"
        );
        assert_eq!(actors.actors[1].kind, ActorKind::System);
        assert!(llm.prompts.lock().unwrap()[0].contains("--- app/ability.rb ---"));

        build_actors(dir.path(), &files, &Surface::default(), &llm, false)
            .await
            .unwrap();
        assert_eq!(llm.prompts.lock().unwrap().len(), 1);

        std::fs::write(dir.path().join("app/ability.rb"), "can :cancel, Contract").unwrap();
        build_actors(dir.path(), &files, &Surface::default(), &llm, false)
            .await
            .unwrap();
        assert_eq!(llm.prompts.lock().unwrap().len(), 2);

        build_actors(dir.path(), &files, &Surface::default(), &llm, true)
            .await
            .unwrap();
        assert_eq!(llm.prompts.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn without_authorization_code_or_user_entities_the_llm_is_not_asked() {
        let dir = tempfile::tempdir().unwrap();
        let llm = ScriptedProvider {
            response: String::new(),
            prompts: Mutex::new(Vec::new()),
        };
        let actors = build_actors(
            dir.path(),
            &paths(&["a.rb"]),
            &Surface::default(),
            &llm,
            false,
        )
        .await
        .unwrap();
        assert!(actors.is_empty());
        assert!(llm.prompts.lock().unwrap().is_empty());
    }
}
