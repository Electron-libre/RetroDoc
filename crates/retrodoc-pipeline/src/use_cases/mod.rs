//! Use cases pass (PLAN.md §2 step 5, roadmap phase 4): for each feature,
//! one LLM call reads the actual code of the feature's files and describes
//! its use cases as ordered steps, each with an actor's name, one text and
//! the code it is grounded on, cited as `"F2:10-24"` (the number of a file of
//! the prompt and a line range), at most 5 observable steps per use case
//! (`issues/shorter_use_case_answers.md`). Saved as the intermediate
//! `.retrodoc/cache/use-cases.yaml` artifact.
//!
//! Grounding is enforced rather than trusted: a step's source reference to a
//! file outside the feature (or to a number the prompt does not show) is dropped, and steps are renumbered from 1 in
//! the order given. Text fields (`description`, `primary_actor`, `narrative`,
//! a step's `text` and `action`) accept an object too, turned into text (its `name`, else
//! its string values), so a wrong shape costs the field, not the feature. As in the features pass, a malformed answer for one
//! feature is logged and that feature skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use futures_util::stream::{self, StreamExt};
use retrodoc_core::config::RulesConfig;
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
use crate::features::{entry_point_id, file_ranges, unique_slug};
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
pub(crate) use self::prompt::{
    focus_for, numbered_excerpt, MAX_CHARS_PER_FILE, MAX_CHARS_PER_PROMPT,
};
use self::prompt::{system_prompt, use_cases_prompt, Framing};

/// How far from an entry point's file the code it runs is followed, and how
/// many files are kept (see [`CodeIndex::slice`]).
const SLICE_DEPTH: usize = 2;
const MAX_SLICE_FILES: usize = 8;

/// Entry points of a feature asked in one call: a feature with more is asked in slices of whole
/// files (a file over the limit is cut), so that no answer is cut at the output limit.
const MAX_ENTRIES_PER_CALL: usize = 12;

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
    /// The vocabulary of a domain, when it has its own (a bounded context read on its own): the
    /// vocabulary of the whole application grows with every context read, which would redo what
    /// was done. A domain without one uses `vocabulary`.
    pub domain_vocabulary: BTreeMap<String, Vec<String>>,
    /// The product brief that frames every prompt (empty: none).
    pub brief: ProductBrief,
    /// The signals the closest extracts of a feature are taken from (empty: none).
    pub evidence: Evidence,
    /// The signals the rules pass takes the closest extracts of a feature from (empty: none). Not
    /// `evidence`: the rules need them even when the features and use cases are not given any.
    pub rule_evidence: Evidence,
    /// What the rules pass must not set aside, as `[rules]` of `retrodoc.toml` lists it.
    pub put_back: RulesConfig,
    /// Features asked at once (`0` and `1`: one at a time). The answers are used in the order of
    /// the features, so the result does not depend on it.
    pub concurrency: usize,
}

impl UseCaseContext {
    /// The vocabulary the prompts and the fingerprints of the features of `domain_slug` use.
    #[must_use]
    pub fn vocabulary_for(&self, domain_slug: &str) -> &[String] {
        self.domain_vocabulary
            .get(domain_slug)
            .unwrap_or(&self.vocabulary)
    }
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
        actors,
        concurrency,
        ..
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

    let items = plan_items(repo_root, features, context, &known, &splitter, llm);

    // The calls go out `concurrency` at a time but the answers come back in the order of the
    // features, and the slugs are made unique in that order: the result is the sequential one.
    let mut answers = stream::iter(items)
        .map(|item| async move {
            let answers = match &item.plan {
                Plan::Ask { requests } => Some(ask_slices(llm, requests, &item.feature.slug).await),
                Plan::Reuse | Plan::Unreadable => None,
            };
            (item, answers)
        })
        .buffered((*concurrency).max(1));

    while let Some((item, answered)) = answers.next().await {
        let Item {
            position,
            feature,
            key,
            print,
            plan,
            ..
        } = item;
        match plan {
            Plan::Reuse => {
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
            }
            Plan::Unreadable => {
                progress.begin(&key);
                tracing::warn!(feature = %feature.slug, "feature skipped: none of its files is readable");
            }
            Plan::Ask { requests } => {
                progress.begin(&key);
                let grounded = ground_slices(
                    feature,
                    &requests,
                    answered.unwrap_or_default(),
                    &use_cases,
                    actors,
                );
                let produced = match grounded {
                    Ok(Some(produced)) => produced,
                    // A slice not answered: the feature is asked again on the next run.
                    Ok(None) => continue,
                    Err(err) => {
                        save_partial(
                            repo_root,
                            &use_cases,
                            &previous,
                            &prints,
                            &known,
                            &features[position..],
                        );
                        return Err(err);
                    }
                };
                prints.use_cases.insert(key, print);
                use_cases.extend(produced);
                // After each answer: a run killed later loses the call in flight, nothing more.
                save_partial(
                    repo_root,
                    &use_cases,
                    &previous,
                    &prints,
                    &known,
                    &features[position + 1..],
                );
            }
        }
    }
    drop(answers);

    // Use cases first: a fingerprint without its use cases would freeze a
    // feature as "none".
    save_use_cases(repo_root, &use_cases)?;
    prints.save(repo_root)?;
    Ok(use_cases)
}

/// The answers to the requests of a feature, one after the other (its answers must not be cut, not
/// hurry); the last one is the one that is not usable, if any.
async fn ask_slices(
    llm: &dyn LlmProvider,
    requests: &[Request],
    feature_slug: &str,
) -> Vec<Result<Answer, PipelineError>> {
    let mut answers = Vec::with_capacity(requests.len());
    for request in requests {
        let answer =
            ask_use_cases(llm, &request.system_prompt, &request.prompt, feature_slug).await;
        let stop = !matches!(answer, Ok(Answer::Use(_) | Answer::Empty));
        answers.push(answer);
        if stop {
            break;
        }
    }
    answers
}

/// The use cases of all the slices of a feature, grounded in order (slugs unique against the
/// `existing` ones and those of the earlier slices). `None` when a slice has no usable answer.
fn ground_slices(
    feature: &Feature,
    requests: &[Request],
    answers: Vec<Result<Answer, PipelineError>>,
    existing: &[UseCase],
    actors: &Actors,
) -> Result<Option<Vec<UseCase>>, PipelineError> {
    let mut produced: Vec<UseCase> = Vec::new();
    for (request, answer) in requests.iter().zip(answers) {
        match answer? {
            Answer::Use(raw) => {
                let taken: Vec<UseCase> = existing.iter().chain(&produced).cloned().collect();
                produced.extend(grounded_use_cases(
                    raw,
                    feature,
                    &taken,
                    &request.input,
                    &request.cited_files,
                    actors,
                ));
            }
            Answer::Empty => {}
            Answer::Unusable => return Ok(None),
        }
    }
    Ok(Some(produced))
}

/// What each feature needs, decided before any call: nothing (unchanged since the last run), or a
/// request.
fn plan_items<'a>(
    repo_root: &Path,
    features: &'a [Feature],
    context: &UseCaseContext,
    known: &BTreeMap<String, String>,
    splitter: &Splitter,
    llm: &dyn LlmProvider,
) -> Vec<Item<'a>> {
    let UseCaseContext {
        entry_points,
        index,
        actors,
        brief,
        evidence,
        ..
    } = context;
    features
        .iter()
        .enumerate()
        .map(|(position, feature)| {
            let key = format!("{}/{}", feature.domain_slug, feature.slug);
            let input = FeatureInput::new(repo_root, feature, entry_points, index);
            let close = evidence.section(&feature_query(feature, &input));
            let framing = Framing {
                actors,
                vocabulary: context.vocabulary_for(&feature.domain_slug),
                brief,
                evidence: &close,
            };
            let print = feature_fingerprint(repo_root, feature, &input, &framing, llm);
            let plan = if known.get(&key) == Some(&print) {
                Plan::Reuse
            } else {
                let requests: Vec<Request> = input
                    .calls(repo_root, feature, index)
                    .into_iter()
                    .filter_map(|input| {
                        let (prompt, cited_files) =
                            use_cases_prompt(repo_root, feature, &input, &framing, splitter);
                        if cited_files.is_empty() {
                            tracing::warn!(feature = %feature.slug, "slice left out: none of its files is readable");
                        }
                        (!cited_files.is_empty()).then(|| Request {
                            system_prompt: system_prompt(
                                !input.entries.is_empty(),
                                !actors.is_empty(),
                            ),
                            prompt,
                            cited_files,
                            input,
                        })
                    })
                    .collect();
                if requests.is_empty() {
                    Plan::Unreadable
                } else {
                    Plan::Ask { requests }
                }
            };
            Item {
                position,
                feature,
                key,
                print,
                plan,
            }
        })
        .collect()
}

/// One feature of the pass: what it is derived from and what is done about it.
struct Item<'a> {
    position: usize,
    feature: &'a Feature,
    key: String,
    print: String,
    plan: Plan,
}

enum Plan {
    /// Unchanged since the last run: its saved use cases are kept.
    Reuse,
    /// None of its files can be read: nothing to ask.
    Unreadable,
    /// One request, or one per slice of a feature with many entry points.
    Ask { requests: Vec<Request> },
}

/// One call: its prompts, and what its answer is grounded on.
struct Request {
    system_prompt: String,
    prompt: String,
    /// The files shown, in the order of their numbers in the prompt.
    cited_files: Vec<String>,
    input: FeatureInput,
}

/// Best-effort save of a run in progress or failed: the `unreached` features
/// keep their previous use cases and fingerprints, so a rerun resumes where
/// this one stopped.
fn save_partial(
    repo_root: &Path,
    done: &[UseCase],
    previous: &[UseCase],
    prints: &Fingerprints,
    known: &BTreeMap<String, String>,
    unreached: &[Feature],
) {
    let mut use_cases = done.to_vec();
    let mut prints = prints.clone();
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
pub(crate) fn feature_query(feature: &Feature, input: &FeatureInput) -> String {
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
#[derive(Clone)]
pub(crate) struct FeatureInput {
    /// The feature's entry points (those it lists, else those defined in its files), with their file.
    pub(crate) entries: Vec<(PathBuf, EntryPoint)>,
    pub(crate) files: Vec<String>,
}

impl FeatureInput {
    pub(crate) fn new(
        repo_root: &Path,
        feature: &Feature,
        entry_points: &EntryPoints,
        index: &CodeIndex,
    ) -> Self {
        let entries: Vec<(PathBuf, EntryPoint)> = entry_points
            .iter()
            .filter(|(file, entry)| {
                if feature.entry_points.is_empty() {
                    feature.source_paths.iter().any(|p| Path::new(p) == *file)
                } else {
                    // A feature from the surface owns its entry points, not all those of its files.
                    feature.entry_points.contains(&entry_point_id(file, entry))
                }
            })
            .map(|(file, entry)| (file.to_path_buf(), entry.clone()))
            .collect();
        Self::of(repo_root, feature, entries, index)
    }

    /// The input of `entries`: the files that define them and the code they run.
    fn of(
        repo_root: &Path,
        feature: &Feature,
        entries: Vec<(PathBuf, EntryPoint)>,
        index: &CodeIndex,
    ) -> Self {
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

    /// The inputs of the calls that derive the use cases: this one, or, for more than
    /// [`MAX_ENTRIES_PER_CALL`] entry points, one per slice of whole files.
    fn calls(self, repo_root: &Path, feature: &Feature, index: &CodeIndex) -> Vec<Self> {
        let files: Vec<&Path> = self.entries.iter().map(|(f, _)| f.as_path()).collect();
        let ranges = file_ranges(&files, MAX_ENTRIES_PER_CALL);
        if ranges.len() <= 1 {
            return vec![self];
        }
        ranges
            .into_iter()
            .map(|range| Self::of(repo_root, feature, self.entries[range].to_vec(), index))
            .collect()
    }
}
