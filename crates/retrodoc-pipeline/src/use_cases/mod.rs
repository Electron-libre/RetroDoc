//! Use cases pass (PLAN.md §2 step 5, roadmap phase 4): for each feature,
//! one LLM call reads the actual code of the feature's files and describes
//! its use cases as ordered steps, each with an actor, an action and the
//! code locations it is grounded on. Saved as the intermediate
//! `.retrodoc/cache/use-cases.yaml` artifact.
//!
//! Grounding is enforced rather than trusted: a step's source reference to a
//! file outside the feature is dropped, and steps are renumbered from 1 in
//! the order given. Text fields (`description`, `primary_actor`, `narrative`,
//! a step's `action`) accept an object too, turned into text (its `name`, else
//! its string values), so a wrong shape costs the field, not the feature. As in the features pass, a malformed answer for one
//! feature is logged and that feature skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_core::model::{Actor, ActorKind, Feature, SourceRef, Step, UseCase};
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::actors::Actors;
use crate::artifact::{load_yaml, save_yaml, warn_on_error, Artifact};
use crate::brief::{Evidence, ProductBrief};
use crate::cache::hash_content;
use crate::chunks::{Focus, Splitter};
use crate::entry_points::{EntryPoint, EntryPoints};
use crate::error::PipelineError;
use crate::features::unique_slug;
use crate::fingerprints::{fingerprint, model_part, Fingerprints};
use crate::progress::Progress;
use crate::response::complete_json;
use crate::roles::load_splitter;
use crate::slices::CodeIndex;

mod grounding;
mod prompt;
#[cfg(test)]
mod tests;

use self::grounding::{grounded_use_cases, RawUseCases};
pub(crate) use self::grounding::{is_human, resolve_cited_path};
pub(crate) use self::prompt::numbered_excerpt;
use self::prompt::{system_prompt, use_cases_prompt, Framing};

/// How far from an entry point's file the code it runs is followed, and how
/// many files are kept (see [`CodeIndex::slice`]).
const SLICE_DEPTH: usize = 2;
const MAX_SLICE_FILES: usize = 8;

/// Loads a previously saved `use-cases.yaml`. Missing or unreadable: `None`
/// (first run), not an error.
#[must_use]
pub fn load_use_cases(repo_root: &Path) -> Option<Vec<UseCase>> {
    load_yaml(&Artifact::UseCases.path(repo_root))
}

/// Persists `use_cases` as `.retrodoc/cache/use-cases.yaml`; also used to
/// re-save once diagrams are attached.
///
/// # Errors
///
/// Returns an error if the file can't be written or serialization fails.
pub fn save_use_cases(repo_root: &Path, use_cases: &[UseCase]) -> Result<(), PipelineError> {
    save_yaml(&Artifact::UseCases.path(repo_root), use_cases)
}

/// What the LLM said about a feature's use cases.
enum Answer {
    Use(RawUseCases),
    /// Two clean answers without any use case: the feature has none, as far as
    /// the model can tell, so asking again on the same input is pointless.
    Empty,
    /// Unparseable, or empty once and unparseable the other time: worth a retry
    /// on the next run.
    Unusable,
}

/// Asks the LLM for the use cases of a feature. An answer without any use
/// case (missing or empty `use_cases`) is as useless as an unparseable one:
/// it is asked once more.
async fn ask_use_cases(
    llm: &dyn LlmProvider,
    system_prompt: &str,
    prompt: &str,
    feature_slug: &str,
) -> Result<Answer, PipelineError> {
    let mut empty_answers = 0;
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
                empty_answers += 1;
                // Only worth a warning once the second answer confirms it.
                tracing::debug!(feature = %feature_slug, attempt, "LLM answered with no use case");
            }
            Some(raw) => return Ok(Answer::Use(raw)),
            None => {}
        }
    }
    Ok(if empty_answers == 2 {
        tracing::warn!(feature = %feature_slug, "LLM answered twice with no use case");
        Answer::Empty
    } else {
        Answer::Unusable
    })
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
    /// The product brief that frames every prompt (empty: none).
    pub brief: ProductBrief,
    /// The signals the closest extracts of a feature are taken from (empty: none).
    pub evidence: Evidence,
}

/// Derives the use cases of every feature from the code it is grounded on
/// and persists them to `.retrodoc/cache/use-cases.yaml`. Diagrams are not
/// attached here (see [`crate::diagrams`]).
///
/// Incremental: a feature whose text and file contents are unchanged since
/// the last run keeps its saved use cases (diagram and confidence included),
/// without an LLM call. That includes a feature the LLM reliably answered
/// "no use case" for (twice, cleanly); `generate --force` or a change of input
/// asks again, and an unparseable answer is always retried.
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
        brief,
        evidence,
    } = context;
    let saved = load_use_cases(repo_root);
    let mut prints = Fingerprints::load(repo_root);
    // Without the saved use cases, a fingerprint can't tell "none" from "lost".
    let known = if saved.is_some() {
        std::mem::take(&mut prints.use_cases)
    } else {
        BTreeMap::new()
    };
    let previous = saved.unwrap_or_default();
    let splitter = load_splitter(repo_root);

    let mut use_cases: Vec<UseCase> = Vec::new();
    let mut progress = Progress::new("use cases", features.len());

    for (position, feature) in features.iter().enumerate() {
        let key = format!("{}/{}", feature.domain_slug, feature.slug);
        let input = FeatureInput::new(repo_root, feature, entry_points, index);
        let close = evidence.section(&feature_query(feature, &input));
        let framing = Framing {
            actors,
            vocabulary,
            brief,
            evidence: &close,
        };
        let print = feature_fingerprint(repo_root, feature, &input, &framing, llm);
        if known.get(&key) == Some(&print) {
            let kept: Vec<UseCase> = previous
                .iter()
                .filter(|u| u.feature_slug == feature.slug)
                .cloned()
                .collect();
            // A saved fingerprint without use cases is a remembered "none".
            tracing::info!(feature = %key, "use cases unchanged, reused");
            use_cases.extend(kept);
            prints.use_cases.insert(key, print);
            progress.skip();
            continue;
        }
        progress.begin(&key);
        let (prompt, cited_files) =
            use_cases_prompt(repo_root, feature, &input, &framing, &splitter);
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
        let produced = match answer {
            Answer::Use(raw) => {
                grounded_use_cases(raw, feature, &use_cases, &input, &cited_files, actors)
            }
            Answer::Empty => Vec::new(),
            Answer::Unusable => continue,
        };
        prints.use_cases.insert(key, print);
        use_cases.extend(produced);
    }

    // Use cases first: a fingerprint without its use cases would freeze a
    // feature as "none".
    save_use_cases(repo_root, &use_cases)?;
    prints.save(repo_root)?;
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
    known: &BTreeMap<String, String>,
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
    warn_on_error(save_use_cases(repo_root, &use_cases));
    warn_on_error(prints.save(repo_root));
}

/// Hash of everything the feature's use cases are derived from: its text and
/// the content of its files (an unreadable file hashes as such, so it
/// invalidates when it becomes readable).
fn feature_fingerprint(
    repo_root: &Path,
    feature: &Feature,
    input: &FeatureInput,
    framing: &Framing,
    llm: &dyn LlmProvider,
) -> String {
    let Framing {
        actors,
        vocabulary,
        brief,
        evidence,
    } = *framing;
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
            .chain(files)
            .chain((!brief.fingerprint().is_empty()).then(|| brief.fingerprint()))
            .chain((!evidence.is_empty()).then(|| evidence.to_string()))
            .chain(std::iter::once(model_part(llm))),
    )
}

/// What the evidence closest to a feature is searched with: its text, its
/// entry points and its files (without their extension).
fn feature_query(feature: &Feature, input: &FeatureInput) -> String {
    let mut query = format!("{} {}", feature.name, feature.description);
    for (_, entry) in &input.entries {
        let _ = write!(query, " {} {}", entry.name, entry.description);
    }
    for path in &input.files {
        let _ = write!(query, " {}", Path::new(path).with_extension("").display());
    }
    query
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
