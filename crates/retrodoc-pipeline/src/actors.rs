//! Business actors (PLAN.md §7.1, phase 7 step 4c): who uses the application,
//! in business terms ("contract manager", "signatory", "e-signature
//! provider") rather than "Developer" or "System". They are derived from the
//! authorization code (abilities, policies, roles, permissions: the files
//! whose name says so) and from the entities of the glossary, in
//! one LLM call, saved as `.retrodoc/cache/actors.yaml`. The use cases pass
//! then names its actors from this list.
//!
//! The list is reused while its inputs (the content of those files and the
//! entities) are unchanged.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_core::model::ActorKind;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::brief::ProductBrief;
use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::fingerprints::fingerprint;
use crate::repo_map::{read_file_lossy, truncate_chars};
use crate::response::complete_json;
use crate::surface::Surface;

/// Authorization files sent to the LLM.
const MAX_AUTH_FILES: usize = 12;
/// Characters kept of each authorization file.
const MAX_AUTH_FILE_CHARS: usize = 3_000;
/// Entities listed in the prompt, the best connected first.
const MAX_ENTITIES: usize = 25;
/// Attributes shown for each of those entities.
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

const ACTORS_SYSTEM_PROMPT: &str = "You are identifying the actors of a software application, in \
business terms. From its authorization code (abilities, policies, roles, permissions) and its \
business entities (most are things, not people: keep only those that stand for someone who acts, \
e.g. a rider, a customer, a member), list who uses it or acts on it: the roles a person can have (e.g. \"Contract \
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
        load_yaml(&Artifact::Actors.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::Actors.path(repo_root), self)
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawActors {
    #[serde(default)]
    actors: Vec<RawActor>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
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
pub(crate) fn words(identifier: &str) -> Vec<String> {
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

/// The business entities, as prompt lines with their main attributes (a
/// `role` or `admin` column is telling). None is filtered out by name: a
/// "rider" or a "courier" is an actor as much as a "user", and only the LLM
/// can tell which entities stand for someone.
fn entity_lines(surface: &Surface) -> Vec<String> {
    surface
        .entities
        .iter()
        .take(MAX_ENTITIES)
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
/// when the authorization files and the entities are unchanged
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
    brief: &ProductBrief,
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
    let entity_lines = entity_lines(surface);
    if contents.is_empty() && entity_lines.is_empty() {
        tracing::info!("no authorization code nor entity found, no actors identified");
        return Ok(Actors::default());
    }

    let input_hash = fingerprint(
        contents
            .iter()
            .map(|(path, content)| format!("{}\n{}", path.display(), hash_content(content)))
            .chain(entity_lines.iter().cloned())
            .chain((!brief.fingerprint().is_empty()).then(|| brief.fingerprint())),
    );
    if !force {
        if let Some(saved) = Actors::load(repo_root).filter(|a| a.input_hash == input_hash) {
            tracing::info!("actors unchanged, reused");
            return Ok(saved);
        }
    }

    let mut prompt = brief.prompt_head();
    if !entity_lines.is_empty() {
        prompt.push_str("Business entities (keep those that stand for someone who acts):\n");
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

    use crate::testing::FakeLlm;

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
        let llm = FakeLlm::answering(
            r#"{"actors":[
                {"name":"Contract manager","kind":"human","description":"Manages contracts","evidence":["app/ability.rb"]},
                {"name":"contract MANAGER","kind":"human"},
                {"name":"E-signature provider","kind":"system","description":"Signs"},
                {"name":" ","kind":"human"}]}"#,
        );

        let actors = build_actors(
            dir.path(),
            &files,
            &Surface::default(),
            &ProductBrief::default(),
            &llm,
            false,
        )
        .await
        .unwrap();
        assert_eq!(actors.actors.len(), 2);
        assert_eq!(
            actors.canonical(" contract manager ").unwrap().name,
            "Contract manager"
        );
        assert_eq!(actors.actors[1].kind, ActorKind::System);
        assert!(llm.prompts()[0].contains("--- app/ability.rb ---"));

        build_actors(
            dir.path(),
            &files,
            &Surface::default(),
            &ProductBrief::default(),
            &llm,
            false,
        )
        .await
        .unwrap();
        assert_eq!(llm.prompts().len(), 1);

        std::fs::write(dir.path().join("app/ability.rb"), "can :cancel, Contract").unwrap();
        build_actors(
            dir.path(),
            &files,
            &Surface::default(),
            &ProductBrief::default(),
            &llm,
            false,
        )
        .await
        .unwrap();
        assert_eq!(llm.prompts().len(), 2);

        build_actors(
            dir.path(),
            &files,
            &Surface::default(),
            &ProductBrief::default(),
            &llm,
            true,
        )
        .await
        .unwrap();
        assert_eq!(llm.prompts().len(), 3);
    }

    #[tokio::test]
    async fn every_entity_is_offered_not_only_those_named_like_a_user() {
        let dir = tempfile::tempdir().unwrap();
        let entity = |name: &str| crate::glossary::MergedEntity {
            name: name.to_string(),
            description: format!("The {name}"),
            attributes: vec!["speed".to_string()],
            associations: Vec::new(),
            files: Vec::new(),
        };
        let surface = Surface {
            entities: vec![entity("Rider"), entity("Order")],
            resources: Vec::new(),
        };
        let llm = FakeLlm::answering(
            r#"{"actors":[{"name":"Rider","kind":"human","description":"Delivers"}]}"#,
        );
        let actors = build_actors(
            dir.path(),
            &paths(&["a.rb"]),
            &surface,
            &ProductBrief::default(),
            &llm,
            false,
        )
        .await
        .unwrap();
        assert_eq!(actors.actors.len(), 1);
        let prompt = &llm.prompts()[0];
        assert!(prompt.contains("- Rider: The Rider"));
        assert!(prompt.contains("- Order: The Order"));
    }

    #[tokio::test]
    async fn without_authorization_code_or_user_entities_the_llm_is_not_asked() {
        let dir = tempfile::tempdir().unwrap();
        let llm = FakeLlm::answering("");
        let actors = build_actors(
            dir.path(),
            &paths(&["a.rb"]),
            &Surface::default(),
            &ProductBrief::default(),
            &llm,
            false,
        )
        .await
        .unwrap();
        assert!(actors.is_empty());
        assert_eq!(llm.prompts(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn the_brief_heads_the_prompt_and_a_changed_brief_asks_again() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();
        std::fs::write(dir.path().join("app/ability.rb"), "can :sign, Contract").unwrap();
        let files = paths(&["app/ability.rb"]);
        let llm = FakeLlm::answering(r#"{"actors":[{"name":"Manager","kind":"human"}]}"#);
        let brief = crate::testing::brief("Sells things to buyers.");

        build_actors(dir.path(), &files, &Surface::default(), &brief, &llm, false)
            .await
            .unwrap();
        let prompt = &llm.prompts()[0];
        assert!(
            prompt.starts_with("Product brief of the application"),
            "{prompt}"
        );

        build_actors(dir.path(), &files, &Surface::default(), &brief, &llm, false)
            .await
            .unwrap();
        assert_eq!(llm.calls(), 1);

        let other = crate::testing::brief("Rents things to renters.");
        build_actors(dir.path(), &files, &Surface::default(), &other, &llm, false)
            .await
            .unwrap();
        assert_eq!(llm.calls(), 2);
    }
}
