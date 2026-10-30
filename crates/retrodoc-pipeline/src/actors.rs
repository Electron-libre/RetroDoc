//! Business actors (docs/findings.md §2, phase 7 step 4c): who uses the application,
//! in business terms ("contract manager", "signatory", "e-signature
//! provider") rather than "Developer" or "System". They are derived from the
//! authorization code (abilities, policies, roles, permissions: the files
//! whose name says so) and from the entities of the glossary, in
//! one LLM call, saved as `.retrodoc/cache/actors.yaml`. The use cases pass
//! then names its actors from this list.
//!
//! The authorization files are read within one prompt budget, in their order
//! of selection: whole when they fit, a long one as the excerpt of its chunks
//! naming an entity or an authorization word (ADR 0027), and the files past
//! the budget are named in a warning (`specs/actors-reading.md`).
//!
//! The list is reused while its inputs (the text sent for those files and the
//! entities) are unchanged.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_core::model::ActorKind;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::brief::ProductBrief;
use crate::chunks::Focus;
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, model_part, prompt_part};
use crate::naming::normalize;
use crate::response::complete_json;
use crate::surface::Surface;
use crate::text::read_file_lossy;

/// Characters of authorization code sent to the LLM, all files together.
const AUTH_CODE_BUDGET: usize = 40_000;
/// Characters of one authorization file past which it is sent as an excerpt.
const AUTH_FILE_BUDGET: usize = 8_000;
/// Below this many characters left, no further file is started.
const MIN_FILE_SHARE: usize = 200;
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

    /// The known actor `name` designates, as spelled in the list: the same
    /// name up to case, else up to separators and a plural, when only one
    /// actor matches.
    #[must_use]
    pub fn canonical(&self, name: &str) -> Option<&BusinessActor> {
        let name = name.trim();
        if let Some(exact) = self
            .actors
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
        {
            return Some(exact);
        }
        let wanted = normalize(name);
        let mut close = self.actors.iter().filter(|a| normalize(&a.name) == wanted);
        match (close.next(), close.next()) {
            (Some(only), None) if !wanted.is_empty() => Some(only),
            _ => None,
        }
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
/// the ones with the most telling word first, then in path order.
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
    found.into_iter().map(|(_, path)| path.clone()).collect()
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

/// The text sent for each authorization file, in order, within
/// [`AUTH_CODE_BUDGET`]: whole when it fits [`AUTH_FILE_BUDGET`], else the
/// excerpt of its chunks naming an entity or an authorization word. The files
/// past the budget are named in one warning.
fn authorization_sections(
    repo_root: &Path,
    files: Vec<PathBuf>,
    surface: &Surface,
) -> Vec<(PathBuf, String)> {
    let splitter = crate::roles::load_splitter(repo_root);
    let focus = Focus {
        terms: surface
            .entities
            .iter()
            .map(|e| e.name.to_lowercase())
            .collect(),
        words: AUTH_WORDS.iter().map(|w| (*w).to_string()).collect(),
        lines: Vec::new(),
    };
    let mut sections = Vec::new();
    let mut unread = Vec::new();
    let mut left = AUTH_CODE_BUDGET;
    for path in files {
        if left < MIN_FILE_SHARE {
            unread.push(path);
            continue;
        }
        let content = match read_file_lossy(repo_root, &path) {
            Ok(content) => content,
            Err(err) => {
                tracing::warn!(path = %path.display(), error = %err, "authorization file skipped");
                continue;
            }
        };
        let text = if content.len() <= AUTH_FILE_BUDGET.min(left) {
            content
        } else {
            splitter.excerpt(&path, &content, AUTH_FILE_BUDGET.min(left), &focus)
        };
        left = left.saturating_sub(text.len());
        sections.push((path, text));
    }
    if let Some(first) = unread.first() {
        tracing::warn!(
            unread = unread.len(),
            first = %first.display(),
            "authorization files left unread by the actors pass, past its prompt budget"
        );
    }
    sections
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
    let contents = authorization_sections(repo_root, authorization_files(source_files), surface);
    let entity_lines = entity_lines(surface);
    if contents.is_empty() && entity_lines.is_empty() {
        tracing::info!("no authorization code nor entity found, no actors identified");
        return Ok(Actors::default());
    }

    let input_hash = fingerprint(
        contents
            .iter()
            .map(|(path, text)| format!("{}\n{text}", path.display()))
            .chain(entity_lines.iter().cloned())
            .chain(brief.print_part())
            .chain(std::iter::once(prompt_part(&[ACTORS_SYSTEM_PROMPT])))
            .chain(std::iter::once(model_part(llm))),
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
    for (path, text) in &contents {
        let _ = write!(prompt, "\n--- {} ---\n{text}\n", path.display());
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

    // spec: ACT-5 (internal: the LLM is faked in-process)
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
        assert_eq!(
            actors.canonical("contract_managers").unwrap().name,
            "Contract manager"
        );
        assert!(actors.canonical("manager").is_none());
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

    fn write(dir: &Path, path: &str, content: &str) {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }

    async fn prompt_for(dir: &Path, files: &[PathBuf], surface: &Surface) -> String {
        let llm = FakeLlm::answering(r#"{"actors":[{"name":"Manager","kind":"human"}]}"#);
        build_actors(dir, files, surface, &ProductBrief::default(), &llm, false)
            .await
            .unwrap();
        llm.prompts().remove(0)
    }

    // spec: ACT-1 (internal: the LLM is faked in-process)
    #[tokio::test]
    async fn every_short_authorization_file_is_sent_whole_past_twelve() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = Vec::new();
        for i in 0..20 {
            let path = format!("app/policies/thing{i:02}_policy.rb");
            write(
                dir.path(),
                &path,
                &format!("class Thing{i:02}Policy\n  def edit?\n    manager?\n  end\nend\n"),
            );
            files.push(PathBuf::from(path));
        }
        let prompt = prompt_for(dir.path(), &files, &Surface::default()).await;
        for i in 0..20 {
            assert!(
                prompt.contains(&format!("class Thing{i:02}Policy")),
                "file {i} missing"
            );
        }
    }

    // spec: ACT-2 (internal: the LLM is faked in-process)
    #[tokio::test]
    async fn a_long_authorization_file_is_sent_as_the_chunks_that_matter() {
        let dir = tempfile::tempdir().unwrap();
        let mut content = String::from("class Roles\n");
        for i in 0..80 {
            let _ = write!(
                content,
                "  def filler_{i}\n    # {}\n    true\n  end\n\n",
                "x".repeat(150)
            );
        }
        content.push_str("  def signatory_can_sign?\n    true\n  end\nend\n");
        assert!(content.len() > 12_000);
        write(dir.path(), "app/models/roles.rb", &content);
        let entity = crate::glossary::MergedEntity {
            name: "Signatory".to_string(),
            description: "Signs contracts".to_string(),
            attributes: Vec::new(),
            associations: Vec::new(),
            files: Vec::new(),
        };
        let surface = Surface {
            entities: vec![entity],
            resources: Vec::new(),
        };
        let prompt = prompt_for(dir.path(), &paths(&["app/models/roles.rb"]), &surface).await;
        assert!(prompt.contains("class Roles"), "{prompt}");
        assert!(prompt.contains("def signatory_can_sign?"), "{prompt}");
        assert!(prompt.contains("omitted)"), "{prompt}");
        assert!(prompt.len() < 10_000, "{}", prompt.len());
    }

    // spec: ACT-3, ACT-4 (internal: the LLM is faked in-process)
    #[tokio::test]
    async fn the_files_past_the_budget_are_left_out_with_one_warning() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = Vec::new();
        for i in 0..60 {
            let path = format!("app/policies/thing{i:02}_policy.rb");
            write(
                dir.path(),
                &path,
                &format!("# Thing{i:02}Policy\n{}\n", "y".repeat(990)),
            );
            files.push(PathBuf::from(path));
        }
        let (logs, _guard) = crate::testing::capture_logs();
        let prompt = prompt_for(dir.path(), &files, &Surface::default()).await;
        assert!(prompt.contains("Thing00Policy"));
        assert!(!prompt.contains("Thing59Policy"));
        let sent = (0..60)
            .filter(|i| prompt.contains(&format!("Thing{i:02}Policy")))
            .count();
        let first_unread = format!("app/policies/thing{sent:02}_policy.rb");
        let logs = logs.text();
        let warnings: Vec<&str> = logs
            .lines()
            .filter(|l| l.contains("WARN") && l.contains("authorization"))
            .collect();
        assert_eq!(warnings.len(), 1, "{logs}");
        assert!(
            warnings[0].contains(&format!("unread={}", 60 - sent)),
            "{logs}"
        );
        assert!(warnings[0].contains(&first_unread), "{logs}");
    }

    // spec: RER-3
    #[tokio::test]
    async fn another_model_identifies_the_actors_again() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();
        std::fs::write(dir.path().join("app/ability.rb"), "can :sign, Contract").unwrap();
        let files = paths(&["app/ability.rb"]);
        let answer = r#"{"actors":[{"name":"Manager","kind":"human"}]}"#;
        let mut calls = Vec::new();
        for model in ["local", "local", "strong"] {
            let llm = FakeLlm::answering(answer).with_model(model);
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
            calls.push(llm.calls());
        }
        assert_eq!(calls, [1, 0, 1]);
    }
}
