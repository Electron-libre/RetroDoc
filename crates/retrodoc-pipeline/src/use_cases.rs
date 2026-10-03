//! Use cases pass (PLAN.md §2 step 5, roadmap phase 4): for each feature,
//! one LLM call reads the actual code of the feature's files and describes
//! its use cases as ordered steps, each with an actor, an action and the
//! code locations it is grounded on. Saved as the intermediate
//! `.retrodoc/cache/use-cases.yaml` artifact.
//!
//! Grounding is enforced rather than trusted: a step's source reference to a
//! file outside the feature is dropped, and steps are renumbered from 1 in
//! the order given. As in the features pass, a malformed answer for one
//! feature is logged and that feature skipped.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_core::model::{Actor, ActorKind, Feature, SourceRef, Step, UseCase};
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::actors::Actors;
use crate::cache::hash_content;
use crate::chunks::{Focus, Splitter};
use crate::entry_points::{EntryPoint, EntryPoints};
use crate::error::PipelineError;
use crate::features::unique_slug;
use crate::fingerprints::{fingerprint, Fingerprints};
use crate::progress::Progress;
use crate::response::complete_json;
use crate::roles::load_splitter;
use crate::slices::CodeIndex;

const USE_CASES_RELATIVE_PATH: &str = ".retrodoc/cache/use-cases.yaml";

/// Per-file truncation and overall budget of code sent for one feature
/// (PLAN.md §6 "cost/volume"); files beyond the budget are left out of the
/// prompt, and so can't be cited.
const MAX_CHARS_PER_FILE: usize = 4000;
const MAX_CHARS_PER_PROMPT: usize = 30_000;

/// How far from an entry point's file the code it runs is followed, and how
/// many files are kept (see [`CodeIndex::slice`]).
const SLICE_DEPTH: usize = 2;
const MAX_SLICE_FILES: usize = 8;

/// Added to the system prompt when the feature has known entry points.
const ENTRY_POINTS_ADDENDUM: &str = " The prompt lists the feature's entry points with their \
observable outputs, then the files that define them and the files those reference. Build each use \
case around one entry point, or a few closely related ones: the actor's goal, then the steps from \
the trigger to the observable outputs, grounded on that code. Add to each use case an \
`entry_points` array with the names of the entry points it covers, copied verbatim from the list. \
Prefer business wording (what happens to the contract, the company, the user) over method names.";

/// Added to the system prompt when the application's business actors are
/// known: they are listed in the prompt and must be used by name.
const ACTORS_ADDENDUM: &str = " The prompt lists the application's known actors. Name each human \
actor with one of them, copied verbatim, choosing the one that fits the step. Name a software \
actor after the real component or external system involved (e.g. an e-signature provider, a mail \
service), and use \"System\" only for the application itself. Give each use case a \
`primary_actor`: the known human actor who triggers it, and begin its steps with the step in \
which that actor acts (submits the request, opens the page, confirms).";

/// Asks for the business-level account of each use case, next to its
/// technical steps (the two output levels).
const NARRATIVE_ADDENDUM: &str = " Also give each use case a `narrative`: two to four sentences \
for a reader who does not know the code, saying who does what and why, which business objects are \
created or changed, and what the observable result is. Use the application's business vocabulary \
(its entities and actors) and do not mention classes, methods, files or HTTP details; those belong \
in the steps.";

const USE_CASES_SYSTEM_PROMPT: &str = "You are documenting a software project from a functional \
point of view. Given a feature and the source code implementing it, describe its use cases: \
concrete scenarios in which an actor achieves a goal with this feature. For each use case give \
the ordered steps, each with its actor (kind \"human\" for a person, \"system\" for a software \
component, service or external system), the action performed, and the file (with line range when \
you can tell) the step is grounded on. Only describe behavior visible in the code provided; do not \
invent steps. Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching \
this shape: {\"use_cases\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\"description\":\"...\",\
\"steps\":[{\"description\":\"...\",\"actor\":{\"name\":\"...\",\"kind\":\"human|system\"},\
\"action\":\"short verb phrase\",\"source_refs\":[{\"path\":\"...\",\"start_line\":1,\
\"end_line\":10}]}]}]}.";

#[derive(Debug, Deserialize)]
struct RawUseCases {
    #[serde(default)]
    use_cases: Vec<RawUseCase>,
}

#[derive(Debug, Deserialize)]
struct RawUseCase {
    slug: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    steps: Vec<RawStep>,
    #[serde(default)]
    entry_points: Vec<String>,
    #[serde(default)]
    primary_actor: Option<String>,
    #[serde(default)]
    narrative: Option<String>,
}

/// Steps are parsed leniently: small models emit empty objects, steps
/// without an actor, or "references" that are just notes. Incomplete steps
/// are dropped and incomplete references ignored, rather than rejecting the
/// whole answer.
#[derive(Debug, Deserialize)]
struct RawStep {
    #[serde(default)]
    description: String,
    actor: Option<RawActor>,
    action: Option<String>,
    #[serde(default)]
    source_refs: Vec<RawSourceRef>,
}

#[derive(Debug, Deserialize)]
struct RawActor {
    name: String,
    #[serde(default)]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct RawSourceRef {
    path: Option<String>,
    start_line: Option<u32>,
    end_line: Option<u32>,
}

/// Loads a previously saved `use-cases.yaml`. Missing or unreadable: `None`
/// (first run), not an error.
#[must_use]
pub fn load_use_cases(repo_root: &Path) -> Option<Vec<UseCase>> {
    let raw = std::fs::read_to_string(repo_root.join(USE_CASES_RELATIVE_PATH)).ok()?;
    serde_yaml::from_str(&raw).ok()
}

/// Persists `use_cases` as `.retrodoc/cache/use-cases.yaml`; also used to
/// re-save once diagrams are attached.
///
/// # Errors
///
/// Returns an error if the file can't be written or serialization fails.
pub fn save_use_cases(repo_root: &Path, use_cases: &[UseCase]) -> Result<(), PipelineError> {
    let path = repo_root.join(USE_CASES_RELATIVE_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| PipelineError::ArtifactIo {
            path: path.clone(),
            source,
        })?;
    }
    let raw = serde_yaml::to_string(use_cases)?;
    std::fs::write(&path, raw).map_err(|source| PipelineError::ArtifactIo { path, source })
}

/// Asks the LLM for the use cases of a feature. An answer without any use
/// case (missing or empty `use_cases`) is as useless as an unparseable one:
/// it is asked once more.
async fn ask_use_cases(
    llm: &dyn LlmProvider,
    system_prompt: &str,
    prompt: &str,
    feature_slug: &str,
) -> Result<Option<RawUseCases>, PipelineError> {
    for attempt in 1..=2 {
        let raw = complete_json::<RawUseCases>(
            llm,
            system_prompt,
            prompt,
            &format!("use cases of {feature_slug}"),
        )
        .await?;
        match raw {
            Some(raw) if raw.use_cases.is_empty() => {
                tracing::warn!(feature = %feature_slug, attempt, "LLM answered with no use case");
            }
            other => return Ok(other),
        }
    }
    Ok(None)
}

/// The system prompt: the base, the narrative request, and the parts that
/// only apply when the feature has entry points / the actors are known.
fn system_prompt(has_entry_points: bool, has_actors: bool) -> String {
    let mut prompt = format!("{USE_CASES_SYSTEM_PROMPT}{NARRATIVE_ADDENDUM}");
    if has_entry_points {
        prompt.push_str(ENTRY_POINTS_ADDENDUM);
    }
    if has_actors {
        prompt.push_str(ACTORS_ADDENDUM);
    }
    prompt
}

/// Everything besides the features and the LLM that the pass draws on: the
/// entry points and the code index (what the use cases start from and the
/// code they run), the business actors and the business vocabulary (what
/// they are told in).
#[derive(Debug, Clone, Default)]
pub struct UseCaseContext {
    pub entry_points: EntryPoints,
    pub index: CodeIndex,
    pub actors: Actors,
    /// Names of the application's main entities.
    pub vocabulary: Vec<String>,
}

/// Derives the use cases of every feature from the code it is grounded on
/// and persists them to `.retrodoc/cache/use-cases.yaml`. Diagrams are not
/// attached here (see [`crate::diagrams`]).
///
/// Incremental: a feature whose text and file contents are unchanged since
/// the last run keeps its saved use cases (diagram and confidence included),
/// without an LLM call.
///
/// # Errors
///
/// Returns an error if an LLM call fails or the artifact can't be saved.
pub async fn build_use_cases(
    repo_root: &Path,
    features: &[Feature],
    context: &UseCaseContext,
    llm: &dyn LlmProvider,
) -> Result<Vec<UseCase>, PipelineError> {
    let UseCaseContext {
        entry_points,
        index,
        actors,
        vocabulary,
    } = context;
    let previous = load_use_cases(repo_root).unwrap_or_default();
    let mut prints = Fingerprints::load(repo_root);
    let known = std::mem::take(&mut prints.use_cases);
    let splitter = load_splitter(repo_root);

    let mut use_cases: Vec<UseCase> = Vec::new();
    let mut progress = Progress::new("use cases", features.len());

    for (position, feature) in features.iter().enumerate() {
        let key = format!("{}/{}", feature.domain_slug, feature.slug);
        let input = FeatureInput::new(repo_root, feature, entry_points, index);
        let print = feature_fingerprint(repo_root, feature, &input, actors, vocabulary);
        if known.get(&key) == Some(&print) {
            let kept: Vec<&UseCase> = previous
                .iter()
                .filter(|u| u.feature_slug == feature.slug)
                .collect();
            if !kept.is_empty() {
                tracing::info!(feature = %key, "use cases unchanged, reused");
                use_cases.extend(kept.into_iter().cloned());
                prints.use_cases.insert(key, print);
                progress.skip();
                continue;
            }
        }
        let produced_before = use_cases.len();
        progress.begin(&key);
        let (prompt, cited_files) =
            use_cases_prompt(repo_root, feature, &input, actors, vocabulary, &splitter);
        if cited_files.is_empty() {
            tracing::warn!(feature = %feature.slug, "feature skipped: none of its files is readable");
            continue;
        }
        let system_prompt = system_prompt(!input.entries.is_empty(), !actors.is_empty());
        let answer = match ask_use_cases(llm, &system_prompt, &prompt, &feature.slug).await {
            Ok(answer) => answer,
            Err(err) => {
                save_partial(
                    repo_root,
                    use_cases,
                    &previous,
                    prints,
                    &known,
                    &features[position..],
                );
                return Err(err);
            }
        };
        let Some(raw) = answer else {
            continue;
        };

        for raw_use_case in raw.use_cases {
            if raw_use_case.steps.is_empty() {
                tracing::warn!(use_case = %raw_use_case.slug, "use case dropped: no steps");
                continue;
            }
            let slug = unique_slug(
                &raw_use_case.slug,
                use_cases
                    .iter()
                    .filter(|u| u.feature_slug == feature.slug)
                    .map(|u| u.slug.as_str()),
            );
            use_cases.push(UseCase {
                slug,
                feature_slug: feature.slug.clone(),
                name: raw_use_case.name,
                description: raw_use_case.description,
                steps: ground_steps(raw_use_case.steps, &cited_files, actors),
                business_language: None,
                entry_points: known_entry_points(&raw_use_case.entry_points, &input.entries),
                narrative: raw_use_case
                    .narrative
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty()),
                primary_actor: raw_use_case
                    .primary_actor
                    .as_deref()
                    .and_then(|name| actors.canonical(name))
                    .map(|actor| actor.name.clone()),
                diagram_mermaid: None,
                confidence: None,
            });
        }
        if use_cases.len() > produced_before {
            prints.use_cases.insert(key, print);
        }
    }

    prints.save(repo_root)?;
    save_use_cases(repo_root, &use_cases)?;
    Ok(use_cases)
}

/// Best-effort save after a failure: the `unreached` features keep their
/// previous use cases and fingerprints, so a rerun resumes where this one
/// stopped.
fn save_partial(
    repo_root: &Path,
    mut use_cases: Vec<UseCase>,
    previous: &[UseCase],
    mut prints: Fingerprints,
    known: &std::collections::BTreeMap<String, String>,
    unreached: &[Feature],
) {
    for feature in unreached {
        let key = format!("{}/{}", feature.domain_slug, feature.slug);
        if let Some(print) = known.get(&key) {
            prints.use_cases.insert(key, print.clone());
        }
        use_cases.extend(
            previous
                .iter()
                .filter(|u| u.feature_slug == feature.slug)
                .cloned(),
        );
    }
    let _ = prints.save(repo_root);
    let _ = save_use_cases(repo_root, &use_cases);
}

/// Hash of everything the feature's use cases are derived from: its text and
/// the content of its files (an unreadable file hashes as such, so it
/// invalidates when it becomes readable).
fn feature_fingerprint(
    repo_root: &Path,
    feature: &Feature,
    input: &FeatureInput,
    actors: &Actors,
    vocabulary: &[String],
) -> String {
    let files = input.files.iter().map(|path| {
        let content = std::fs::read(repo_root.join(path)).map_or_else(
            |_| "unreadable".to_string(),
            |bytes| hash_content(&String::from_utf8_lossy(&bytes)),
        );
        format!("{path}\n{content}")
    });
    fingerprint(
        [feature.name.clone(), feature.description.clone()]
            .into_iter()
            .chain(input.entries.iter().map(|(_, entry)| {
                let outputs: Vec<&str> = entry
                    .outputs
                    .iter()
                    .map(|o| o.description.as_str())
                    .collect();
                format!(
                    "entry {}\n{}\n{}",
                    entry.name,
                    entry.description,
                    outputs.join("|")
                )
            }))
            .chain(actors.fingerprint_parts())
            .chain(std::iter::once(format!(
                "vocabulary {}",
                vocabulary.join(",")
            )))
            .chain(files),
    )
}

/// Numbers steps from 1 (skipping incomplete ones) and keeps only the source
/// references that point to a file actually shown to the LLM for this
/// feature.
fn ground_steps(raw_steps: Vec<RawStep>, allowed: &BTreeSet<String>, actors: &Actors) -> Vec<Step> {
    let complete = raw_steps.into_iter().filter_map(|raw| {
        let (Some(actor), Some(action)) = (raw.actor, raw.action) else {
            tracing::warn!("step dropped: no actor or no action");
            return None;
        };
        Some((raw.description, actor, action, raw.source_refs))
    });
    complete
        .zip(1..)
        .map(|((description, actor, action, refs), order)| {
            let source_refs = refs
                .into_iter()
                .filter_map(|r| {
                    let cited = r.path?;
                    let Some(path) = resolve_cited_path(&cited, allowed) else {
                        tracing::warn!(path = %cited, "step reference dropped: file not in the feature");
                        return None;
                    };
                    Some(SourceRef {
                        path,
                        start_line: r.start_line,
                        end_line: r.end_line,
                    })
                })
                .collect();
            Step {
                order,
                description,
                actor: resolve_actor(&actor, actors),

                action,
                source_refs,
            }
        })
        .collect()
}

/// The step's actor as the known list spells it (name and kind), when the
/// LLM named a known one. A human actor outside a non-empty list is kept but
/// reported: the list is the vocabulary the use cases should use.
fn resolve_actor(raw: &RawActor, actors: &Actors) -> Actor {
    if let Some(known) = actors.canonical(&raw.name) {
        return Actor {
            name: known.name.clone(),
            kind: known.kind,
        };
    }
    let kind = if raw.kind.eq_ignore_ascii_case("human") {
        ActorKind::Human
    } else {
        ActorKind::System
    };
    if kind == ActorKind::Human && !actors.is_empty() {
        tracing::warn!(actor = %raw.name, "human actor outside the known actors");
    }
    Actor {
        name: raw.name.clone(),
        kind,
    }
}

/// Maps a path cited by the LLM to the feature file it designates: an exact
/// match, or a unique file the citation is a path suffix of (or the other way
/// round). Models often shorten `crate/src/a.rs` to `src/a.rs`.
pub(crate) fn resolve_cited_path(cited: &str, allowed: &BTreeSet<String>) -> Option<String> {
    let cited = cited.trim().trim_start_matches("./");
    if allowed.contains(cited) {
        return Some(cited.to_string());
    }
    let mut matches = allowed.iter().filter(|file| {
        file.ends_with(&format!("/{cited}")) || cited.ends_with(&format!("/{file}"))
    });
    match (matches.next(), matches.next()) {
        (Some(only), None) => Some(only.clone()),
        _ => None,
    }
}

/// What a feature's use cases are derived from: its entry points and the
/// files shown to the LLM. A feature with entry points shows the files that
/// define them and the code they run (a [`CodeIndex::slice`]); without any,
/// all the feature's files, as before.
struct FeatureInput {
    /// Entry points defined in the feature's files, with their file.
    entries: Vec<(PathBuf, EntryPoint)>,
    files: Vec<String>,
}

impl FeatureInput {
    fn new(
        repo_root: &Path,
        feature: &Feature,
        entry_points: &EntryPoints,
        index: &CodeIndex,
    ) -> Self {
        let entries: Vec<(PathBuf, EntryPoint)> = entry_points
            .iter()
            .filter(|(file, _)| feature.source_paths.iter().any(|p| Path::new(p) == *file))
            .map(|(file, entry)| (file.to_path_buf(), entry.clone()))
            .collect();
        if entries.is_empty() {
            return Self {
                entries,
                files: feature.source_paths.clone(),
            };
        }
        let mut starts: Vec<PathBuf> = Vec::new();
        for (file, _) in &entries {
            if !starts.contains(file) {
                starts.push(file.clone());
            }
        }
        let slice = index.slice(repo_root, &starts, SLICE_DEPTH, MAX_SLICE_FILES);
        let files = starts
            .iter()
            .chain(&slice)
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        Self { entries, files }
    }
}

/// The entry point names of an answer that designate a known entry point
/// (exact, else case-insensitive), as the inventory spells them.
fn known_entry_points(cited: &[String], entries: &[(PathBuf, EntryPoint)]) -> Vec<String> {
    let mut known: Vec<String> = Vec::new();
    for name in cited {
        let found = entries
            .iter()
            .map(|(_, entry)| entry.name.as_str())
            .find(|n| *n == name || n.eq_ignore_ascii_case(name.trim()));
        match found {
            Some(found) if !known.iter().any(|k| k == found) => known.push(found.to_string()),
            Some(_) => {}
            None => {
                tracing::warn!(entry_point = %name, "use case cites an unknown entry point, dropped");
            }
        }
    }
    known
}

/// Builds the user prompt for `feature` from the code of its files, within
/// the prompt budget. Returns it with the set of files actually included.
fn use_cases_prompt(
    repo_root: &Path,
    feature: &Feature,
    input: &FeatureInput,
    actors: &Actors,
    vocabulary: &[String],
    splitter: &Splitter,
) -> (String, BTreeSet<String>) {
    let focus = focus_for(feature, &input.entries);
    let mut prompt = format!("Feature: {} — {}\n", feature.name, feature.description);
    if !vocabulary.is_empty() {
        let _ = write!(
            prompt,
            "\nBusiness vocabulary (main entities): {}\n",
            vocabulary.join(", ")
        );
    }
    if !actors.is_empty() {
        let _ = write!(prompt, "\nKnown actors:\n{}", actors.prompt_section());
    }
    if !input.entries.is_empty() {
        prompt.push_str("\nEntry points:\n");
        for (file, entry) in &input.entries {
            let _ = write!(
                prompt,
                "- {} ({}): {}",
                entry.name,
                file.display(),
                entry.description
            );
            let outputs: Vec<String> = entry
                .outputs
                .iter()
                .map(|o| format!("{:?} {}", o.kind, o.description))
                .collect();
            if !outputs.is_empty() {
                let _ = write!(prompt, " → outputs: {}", outputs.join("; "));
            }
            prompt.push('\n');
        }
    }
    prompt.push_str("\nSource files:\n");
    let mut included = BTreeSet::new();
    let mut budget = MAX_CHARS_PER_PROMPT;

    for path in &input.files {
        if budget == 0 {
            break;
        }
        let Ok(bytes) = std::fs::read(repo_root.join(path)) else {
            tracing::warn!(path = %path, "could not read a feature file, left out of the prompt");
            continue;
        };
        let content = String::from_utf8_lossy(&bytes);
        let excerpt = splitter.excerpt(
            Path::new(path),
            &content,
            MAX_CHARS_PER_FILE.min(budget),
            &focus,
        );
        budget = budget.saturating_sub(excerpt.len());
        let _ = write!(prompt, "\n=== {path} ===\n{excerpt}\n");
        included.insert(path.clone());
    }
    (prompt, included)
}

/// What a long file's excerpt should favour for `feature`: the identifiers
/// its entry points are named after (`send_contract` in `POST
/// /contracts/:id/send_contract`), then the words of its own text.
fn focus_for(feature: &Feature, entries: &[(PathBuf, EntryPoint)]) -> Focus {
    /// Too common to point at any code.
    const NOISE: &[&str] = &["post", "patch", "delete", "head", "implied", "callback"];
    let tokens = |text: &str, min_len: usize| -> Vec<String> {
        text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| w.len() >= min_len)
            .map(str::to_lowercase)
            .filter(|w| !NOISE.contains(&w.as_str()))
            .collect()
    };
    let mut terms: Vec<String> = Vec::new();
    for (_, entry) in entries {
        for term in tokens(&entry.name, 4)
            .into_iter()
            .chain(tokens(&entry.verb, 4))
        {
            if !terms.contains(&term) {
                terms.push(term);
            }
        }
    }
    let mut words: Vec<String> = Vec::new();
    for word in tokens(&format!("{} {}", feature.name, feature.description), 5) {
        if !terms.contains(&word) && !words.contains(&word) {
            words.push(word);
        }
    }
    Focus {
        terms,
        words,
        lines: Vec::new(),
    }
}

/// Prefixes each line with its 1-based number (so the LLM can cite line
/// ranges) and stops once `max_chars` are reached.
pub(crate) fn numbered_excerpt(content: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (n, line) in content.lines().enumerate() {
        if out.len() >= max_chars {
            out.push_str("… (truncated)\n");
            break;
        }
        let _ = writeln!(out, "{:>4} | {line}", n + 1);
    }
    out
}

/// Whether the actor is a person rather than a software component.
#[must_use]
pub(crate) fn is_human(actor: &Actor) -> bool {
    actor.kind == ActorKind::Human
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;
    use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError};

    struct CannedProvider {
        response: String,
    }

    #[async_trait]
    impl LlmProvider for CannedProvider {
        async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            Ok(CompletionResponse {
                content: self.response.clone(),
                model: "test-model".to_string(),
            })
        }
    }

    fn feature(slug: &str, paths: &[&str]) -> Feature {
        Feature {
            slug: slug.to_string(),
            domain_slug: "billing".to_string(),
            sub_domain_slug: None,
            name: slug.to_string(),
            description: "d".to_string(),
            source_paths: paths.iter().map(ToString::to_string).collect(),
            confidence: None,
        }
    }

    #[test]
    fn cited_paths_resolve_to_a_unique_feature_file() {
        let allowed: BTreeSet<String> = ["m/src/schema.rs", "m/src/spec.rs", "x/src/spec.rs"]
            .map(String::from)
            .into();
        let resolve = |c| resolve_cited_path(c, &allowed);
        assert_eq!(
            resolve("m/src/schema.rs").as_deref(),
            Some("m/src/schema.rs")
        );
        assert_eq!(
            resolve("./src/schema.rs").as_deref(),
            Some("m/src/schema.rs")
        );
        assert_eq!(resolve("src/spec.rs"), None); // ambiguous
        assert_eq!(resolve("src/other.rs"), None);
    }

    #[test]
    fn numbered_excerpt_numbers_lines_and_truncates() {
        let content = "a\nb\nc\n";
        assert_eq!(
            numbered_excerpt(content, 1000),
            "   1 | a\n   2 | b\n   3 | c\n"
        );
        assert!(numbered_excerpt(content, 1).contains("(truncated)"));
    }

    #[tokio::test]
    async fn build_use_cases_renumbers_steps_and_drops_ungrounded_refs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        let provider = CannedProvider {
            response: r#"{"use_cases":[
              {"slug":"Pay invoice","name":"Pay invoice","description":"d","steps":[
                {"description":"s1","actor":{"name":"Customer","kind":"human"},
                 "action":"submits payment",
                 "source_refs":[{"path":"a.rs","start_line":1,"end_line":1},
                                {"path":"ghost.rs","start_line":null,"end_line":null}]},
                {"description":"s2","actor":{"name":"API","kind":"system"},
                 "action":"records payment","source_refs":[]}]},
              {"slug":"empty","name":"Empty","description":"d","steps":[]}
            ]}"#
            .to_string(),
        };

        let use_cases = build_use_cases(
            dir.path(),
            &[feature("payment", &["a.rs"])],
            &UseCaseContext::default(),
            &provider,
        )
        .await
        .unwrap();

        assert_eq!(use_cases.len(), 1);
        let uc = &use_cases[0];
        assert_eq!(uc.slug, "pay-invoice");
        assert_eq!(uc.feature_slug, "payment");
        assert_eq!(
            uc.steps.iter().map(|s| s.order).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(uc.steps[0].source_refs.len(), 1);
        assert_eq!(uc.steps[0].source_refs[0].path, "a.rs");
        assert!(is_human(&uc.steps[0].actor));
        assert!(!is_human(&uc.steps[1].actor));

        assert_eq!(load_use_cases(dir.path()).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn build_use_cases_skips_features_without_readable_files_and_bad_answers() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        let provider = CannedProvider {
            response: "not json".to_string(),
        };

        let use_cases = build_use_cases(
            dir.path(),
            &[
                feature("missing", &["nope.rs"]),
                feature("garbled", &["a.rs"]),
            ],
            &UseCaseContext::default(),
            &provider,
        )
        .await
        .unwrap();

        assert!(use_cases.is_empty());
    }

    #[tokio::test]
    async fn build_use_cases_tolerates_sloppy_steps_and_references() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        // Two fenced blocks (only the first counts), an empty step, a step
        // without actor, a note instead of a reference, a free-form actor kind.
        let provider = CannedProvider {
            response: r#"```json
            {"use_cases":[{"slug":"u","name":"U","description":"d","steps":[
              {},
              {"description":"no actor","action":"x"},
              {"description":"ok","actor":{"name":"Dev","kind":"Human"},"action":"does",
               "source_refs":[{"note":"inferred"},{"path":"a.rs"}]}]}]}
            ```
            ```json
            {"use_cases":[]}
            ```"#
                .to_string(),
        };

        let use_cases = build_use_cases(
            dir.path(),
            &[feature("f", &["a.rs"])],
            &UseCaseContext::default(),
            &provider,
        )
        .await
        .unwrap();

        assert_eq!(use_cases.len(), 1);
        let steps = &use_cases[0].steps;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].order, 1);
        assert!(is_human(&steps[0].actor));
        assert_eq!(steps[0].source_refs.len(), 1);
        assert_eq!(steps[0].source_refs[0].path, "a.rs");
    }

    struct CountingProvider {
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl LlmProvider for CountingProvider {
        async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(CompletionResponse {
                content: r#"{"use_cases":[{"slug":"u","name":"U","description":"d","steps":[
                {"description":"s","actor":{"name":"A","kind":"human"},"action":"act",
                 "source_refs":[{"path":"a.rs"}]}]}]}"#
                    .to_string(),
                model: "test-model".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn rerun_reuses_use_cases_until_a_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
        let features = vec![feature("pay", &["a.rs"])];
        let provider = CountingProvider {
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        let calls = || provider.calls.load(std::sync::atomic::Ordering::SeqCst);

        let mut first =
            build_use_cases(dir.path(), &features, &UseCaseContext::default(), &provider)
                .await
                .unwrap();
        assert_eq!(calls(), 1);
        // Scored use cases keep their score when reused.
        first[0].confidence = Some(retrodoc_core::model::ConfidenceScore::new(0.9, None));
        save_use_cases(dir.path(), &first).unwrap();

        let again = build_use_cases(dir.path(), &features, &UseCaseContext::default(), &provider)
            .await
            .unwrap();
        assert_eq!(calls(), 1, "unchanged feature must not call the LLM");
        assert_eq!(again.len(), 1);
        assert!(again[0].confidence.is_some());

        std::fs::write(dir.path().join("a.rs"), "fn a() { changed }").unwrap();
        let redone = build_use_cases(dir.path(), &features, &UseCaseContext::default(), &provider)
            .await
            .unwrap();
        assert_eq!(calls(), 2, "a changed file must invalidate the feature");
        assert!(redone[0].confidence.is_none());
    }

    /// Answers a fixed use case and records the prompts it receives.
    struct RecordingProvider {
        response: String,
        prompts: std::sync::Mutex<Vec<(String, String)>>,
    }

    #[async_trait]
    impl LlmProvider for RecordingProvider {
        async fn complete(
            &self,
            request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.prompts.lock().unwrap().push((
                request.messages[0].content.clone(),
                request.messages[1].content.clone(),
            ));
            Ok(CompletionResponse {
                content: self.response.clone(),
                model: "m".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn a_feature_with_entry_points_gets_them_and_the_code_they_run() {
        use crate::entry_points::{EntryFile, EntryKind, Output, OutputKind};
        use std::collections::BTreeMap;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (path, content) in [
            (
                "app/contracts_controller.rb",
                "def sign\n  ContractSigner.call\nend\n",
            ),
            ("app/contract_signer.rb", "class ContractSigner\nend\n"),
            ("app/unrelated.rb", "class Unrelated\nend\n"),
        ] {
            std::fs::create_dir_all(root.join("app")).unwrap();
            std::fs::write(root.join(path), content).unwrap();
        }
        let entry_points = EntryPoints {
            files: BTreeMap::from([(
                PathBuf::from("app/contracts_controller.rb"),
                EntryFile {
                    content_hash: String::new(),
                    entry_points: vec![EntryPoint {
                        kind: EntryKind::HttpRoute,
                        name: "POST /contracts/:id/sign".to_string(),
                        verb: "sign".to_string(),
                        resource: "contract".to_string(),
                        description: "A signatory signs".to_string(),
                        outputs: vec![Output {
                            kind: OutputKind::Email,
                            description: "confirmation sent".to_string(),
                        }],
                    }],
                },
            )]),
        };
        let index = CodeIndex::new(
            [
                "app/contracts_controller.rb",
                "app/contract_signer.rb",
                "app/unrelated.rb",
            ]
            .iter()
            .map(Path::new),
        );
        let provider = RecordingProvider {
            response: r#"{"use_cases":[{"slug":"sign","name":"Sign a contract","description":"d",
              "entry_points":["post /contracts/:id/sign","GET /ghost"],
              "steps":[{"description":"s","actor":{"name":"Signatory","kind":"human"},
                "action":"signs","source_refs":[{"path":"app/contract_signer.rb"}]}]}]}"#
                .to_string(),
            prompts: std::sync::Mutex::new(Vec::new()),
        };
        // The feature only lists the controller: the signer is outside it.
        let features = [feature("signing", &["app/contracts_controller.rb"])];

        let context = UseCaseContext {
            entry_points,
            index,
            ..UseCaseContext::default()
        };
        let use_cases = build_use_cases(root, &features, &context, &provider)
            .await
            .unwrap();

        let prompts = provider.prompts.lock().unwrap();
        assert!(prompts[0].0.contains("entry_points"));
        assert!(prompts[0].1.contains(
            "- POST /contracts/:id/sign (app/contracts_controller.rb): A signatory signs"
        ));
        assert!(prompts[0].1.contains("Email confirmation sent"));
        assert!(prompts[0].1.contains("=== app/contract_signer.rb ==="));
        assert!(!prompts[0].1.contains("unrelated.rb"));
        // Known entry points are kept as the inventory spells them, unknown dropped;
        // a step may cite code reached through the entry point.
        assert_eq!(use_cases[0].entry_points, vec!["POST /contracts/:id/sign"]);
        assert_eq!(
            use_cases[0].steps[0].source_refs[0].path,
            "app/contract_signer.rb"
        );
    }

    #[tokio::test]
    async fn known_actors_reach_the_prompt_and_name_the_steps() {
        use crate::actors::BusinessActor;

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        let actors = Actors {
            input_hash: String::new(),
            actors: vec![
                BusinessActor {
                    name: "Signatory".to_string(),
                    kind: ActorKind::Human,
                    description: "Signs contracts".to_string(),
                    evidence: Vec::new(),
                },
                BusinessActor {
                    name: "E-signature provider".to_string(),
                    kind: ActorKind::System,
                    description: "Collects signatures".to_string(),
                    evidence: Vec::new(),
                },
            ],
        };
        let provider = RecordingProvider {
            response: r#"{"use_cases":[{"slug":"sign","name":"Sign","description":"d","primary_actor":"SIGNATORY","narrative":"  A signatory signs the contract.  ","steps":[
              {"description":"s1","actor":{"name":"signatory","kind":"system"},"action":"signs"},
              {"description":"s2","actor":{"name":"e-signature PROVIDER","kind":"human"},"action":"records"},
              {"description":"s3","actor":{"name":"Developer","kind":"human"},"action":"reads"}]}]}"#
                .to_string(),
            prompts: std::sync::Mutex::new(Vec::new()),
        };

        let use_cases = build_use_cases(
            dir.path(),
            &[feature("signing", &["a.rs"])],
            &UseCaseContext {
                actors,
                ..UseCaseContext::default()
            },
            &provider,
        )
        .await
        .unwrap();

        let prompts = provider.prompts.lock().unwrap();
        assert!(prompts[0].0.contains("known actors"));
        assert!(prompts[0]
            .1
            .contains("- Signatory (human): Signs contracts"));
        let steps = &use_cases[0].steps;
        // Known actors take the list's spelling *and* kind; others are kept as given.
        assert_eq!(
            (steps[0].actor.name.as_str(), steps[0].actor.kind),
            ("Signatory", ActorKind::Human)
        );
        assert_eq!(
            (steps[1].actor.name.as_str(), steps[1].actor.kind),
            ("E-signature provider", ActorKind::System)
        );
        assert_eq!(steps[2].actor.name, "Developer");
        // The primary actor must be a known one, spelled as in the list.
        assert_eq!(use_cases[0].primary_actor.as_deref(), Some("Signatory"));
        // The narrative is stored trimmed; the prompt asks for it and for the vocabulary.
        assert_eq!(
            use_cases[0].narrative.as_deref(),
            Some("A signatory signs the contract.")
        );
        assert!(prompts[0].0.contains("`narrative`"));
    }

    /// Answers the first `succeed` calls, then fails; counts its calls.
    struct FlakyProvider {
        succeed: usize,
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl LlmProvider for FlakyProvider {
        async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n < self.succeed {
                Ok(CompletionResponse {
                    content: format!(
                        r#"{{"use_cases":[{{"slug":"uc{n}","name":"U","description":"d","steps":[
                        {{"description":"s","actor":{{"name":"A","kind":"human"}},"action":"x","source_refs":[]}}]}}]}}"#
                    ),
                    model: "test-model".to_string(),
                })
            } else {
                Err(LlmError::Transport("down".to_string()))
            }
        }
    }

    #[tokio::test]
    async fn a_failed_run_keeps_the_features_done_and_the_rerun_resumes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "fn b() {}\n").unwrap();
        let features = [feature("one", &["a.rs"]), feature("two", &["b.rs"])];
        let context = UseCaseContext::default();

        let flaky = FlakyProvider {
            succeed: 1,
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        assert!(build_use_cases(dir.path(), &features, &context, &flaky)
            .await
            .is_err());
        assert_eq!(load_use_cases(dir.path()).unwrap().len(), 1);

        let healthy = FlakyProvider {
            succeed: usize::MAX,
            calls: std::sync::atomic::AtomicUsize::new(1),
        };
        let use_cases = build_use_cases(dir.path(), &features, &context, &healthy)
            .await
            .unwrap();
        assert_eq!(use_cases.len(), 2);
        // Only the second feature was sent to the LLM again.
        assert_eq!(healthy.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_long_controller_is_shown_around_the_actions_of_its_entry_points() {
        use crate::entry_points::{EntryFile, EntryKind};
        use std::collections::BTreeMap;
        use std::fmt::Write as _;

        let dir = tempfile::tempdir().unwrap();
        let mut code = String::from("class ContractsController\n\n");
        for i in 1..=60 {
            let name = if i == 55 {
                "send_contract".to_string()
            } else {
                format!("action_{i}")
            };
            let _ = writeln!(code, "  def {name}");
            for step in 1..=8 {
                let _ = writeln!(code, "    work_{i}_{step}");
            }
            let _ = writeln!(code, "  end\n");
        }
        std::fs::write(dir.path().join("contracts_controller.rb"), &code).unwrap();
        let entry_points = EntryPoints {
            files: BTreeMap::from([(
                PathBuf::from("contracts_controller.rb"),
                EntryFile {
                    content_hash: String::new(),
                    entry_points: vec![EntryPoint {
                        kind: EntryKind::HttpRoute,
                        name: "POST /contracts/:id/send_contract".to_string(),
                        verb: "send".to_string(),
                        resource: "contract".to_string(),
                        description: "Sends the contract".to_string(),
                        outputs: Vec::new(),
                    }],
                },
            )]),
        };
        let context = UseCaseContext {
            entry_points,
            index: CodeIndex::new([Path::new("contracts_controller.rb")]),
            ..UseCaseContext::default()
        };
        let provider = RecordingProvider {
            response: r#"{"use_cases":[]}"#.to_string(),
            prompts: std::sync::Mutex::new(Vec::new()),
        };
        let features = [feature("sending", &["contracts_controller.rb"])];

        build_use_cases(dir.path(), &features, &context, &provider)
            .await
            .unwrap();

        let prompts = provider.prompts.lock().unwrap();
        let prompt = &prompts[0].1;
        assert!(prompt.contains("def send_contract"), "the action is shown");
        assert!(
            prompt.contains("class ContractsController"),
            "so is the header"
        );
        assert!(prompt.contains("omitted)"));
        assert!(!prompt.contains("def action_20"));
    }
}
