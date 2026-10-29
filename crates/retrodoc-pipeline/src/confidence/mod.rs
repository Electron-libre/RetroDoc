//! Confidence pass (PLAN.md §2 step 7, roadmap phase 5): a cross-check of
//! each use case against the code it cites. One LLM call per use case gets
//! the steps and the excerpts of the cited files and must give a verdict per
//! step (`supported`, `partial`, `unsupported`), which is turned into a
//! [`ConfidenceScore`].
//!
//! Scoring is deliberately not left to the model alone. Each step is first
//! checked mechanically ([`checks`]: the cited lines exist, the identifiers
//! and entities it names are in the cited code). A step that passes is
//! supported and one whose cited code is not there is unsupported, both
//! without asking; only the doubtful steps go to the model, plus the whole
//! of `--confidence-sample` use cases that pass, for a second opinion. Then:
//! - a step citing no code is capped at [`UNGROUNDED_STEP_CAP`], whatever the
//!   verdict;
//! - a use case whose steps cite no readable code at all is scored 0 without
//!   an LLM call;
//! - a feature's score is the mean of its use cases' scores (0 if it has
//!   none), so a feature is never more trustworthy than what it documents.
//!
//! A use case whose verdict can't be parsed stays unscored (`None`) rather
//! than getting an invented number; the report lists it as such.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use futures_util::stream::{self, StreamExt};
use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

mod checks;

use crate::artifact::warn_on_error;
use crate::chunks::{Focus, Splitter};
use crate::error::PipelineError;
use crate::features::save_features;
use crate::fingerprints::{prompt_part, Fingerprints};
use crate::glossary::Glossary;
use crate::progress::Progress;
use crate::response::complete_json;
use crate::roles::load_splitter;
use crate::use_cases::save_use_cases;
use crate::walked::WalkedFiles;
use checks::{check_step, StepCheck};

/// Highest score a step without any source reference can get.
const UNGROUNDED_STEP_CAP: f32 = 0.25;

/// Use cases of a feature judged in one request.
const MAX_USE_CASES_PER_REQUEST: usize = 5;

/// Characters shown of each cited file.
const MAX_CHARS_PER_FILE: usize = 4_000;
/// Characters of code in one request; the cited files share it.
const MAX_CHARS_PER_PROMPT: usize = 30_000;

const CONFIDENCE_SYSTEM_PROMPT: &str = "You are fact-checking documentation against source code. \
You are given a use case, its numbered steps, and the code the steps cite. For each step decide \
whether the code provided actually shows it happening: \"supported\" (the code clearly does this), \
\"partial\" (plausible but only partly visible, or details are guessed) or \"unsupported\" (nothing \
in the code shows it). Be strict: do not give credit for what is merely plausible. Give a \
rationale ONLY for steps that are not supported: at most 15 words, plain prose, no code and no \
double quotes (an empty string for supported steps). Do not think out loud or explain outside the \
JSON. Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching this \
shape: {\"steps\":[{\"order\":1,\"verdict\":\"supported|partial|unsupported\",\
\"rationale\":\"...\"}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawVerdicts {
    #[serde(default)]
    steps: Vec<RawVerdict>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawBatchVerdicts {
    #[serde(default)]
    use_cases: Vec<RawUseCaseVerdicts>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawUseCaseVerdicts {
    slug: String,
    #[serde(default)]
    steps: Vec<RawVerdict>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
struct RawVerdict {
    order: u32,
    #[serde(default)]
    verdict: String,
    #[serde(default)]
    rationale: String,
}

/// Verdict → score; anything unrecognized counts as unsupported.
fn verdict_value(verdict: &str) -> f32 {
    match verdict.trim().to_ascii_lowercase().as_str() {
        "supported" => 1.0,
        "partial" => 0.5,
        _ => 0.0,
    }
}

/// How the mechanical checks judge the steps of one use case, for tools that
/// compare them with a model's verdict (`examples/confidence_agreement.rs`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StepTally {
    pub passing: usize,
    pub ungrounded: usize,
    pub doubtful: usize,
    pub broken: usize,
}

/// Checks the steps of `use_case` without a model. The score is `Some` when
/// the checks settle every step (what `score_confidence` would give without
/// asking), `None` when a doubtful step is left to the model.
#[must_use]
pub fn mechanical_score(
    walked: &WalkedFiles,
    use_case: &UseCase,
    entities: &[String],
) -> (StepTally, Option<ConfidenceScore>) {
    let checks: Vec<StepCheck> = use_case
        .steps
        .iter()
        .map(|s| check_step(walked, s, entities))
        .collect();
    let mut tally = StepTally::default();
    for check in &checks {
        match check {
            StepCheck::Pass => tally.passing += 1,
            StepCheck::Ungrounded => tally.ungrounded += 1,
            StepCheck::Doubtful(_) => tally.doubtful += 1,
            StepCheck::Broken(_) => tally.broken += 1,
        }
    }
    let score = (tally.doubtful == 0).then(|| combine(use_case, &[], &checks));
    (tally, score)
}

/// `count` of `items`, evenly spaced.
fn evenly_spaced(items: &[usize], count: usize) -> Vec<usize> {
    let count = count.min(items.len());
    let total = items.len().max(1);
    items
        .iter()
        .enumerate()
        .filter(|(rank, _)| rank * count / total != (rank + 1) * count / total)
        .map(|(_, &i)| i)
        .collect()
}

/// Scores every use case not scored yet (reused use cases keep their score
/// from the last run): by the mechanical checks where they settle every step,
/// by the model for the doubtful steps, and for `sample` of the use cases that
/// pass every check, evenly spread, as a second opinion. Then every feature, and persists both artifacts
/// (`use-cases.yaml`, `features.yaml`) with their `confidence` filled in. `concurrency` groups of use
/// cases are checked at a time; the scores are the same as one at a time. A cited file that is not one of
/// `walked` is unreadable, as a missing one is: `use-cases.yaml` can be edited.
///
/// # Errors
///
/// Returns an error if an LLM call fails or an artifact can't be saved.
pub async fn score_confidence(
    walked: &WalkedFiles,
    features: &mut [Feature],
    use_cases: &mut [UseCase],
    llm: &dyn LlmProvider,
    sample: Option<usize>,
    concurrency: usize,
) -> Result<(), PipelineError> {
    let repo_root = walked.root();
    let splitter = load_splitter(repo_root);
    // Scores given by another model are not kept: it scores them again.
    let mut prints = Fingerprints::load(repo_root);
    if prints.confidence_model != Some(scored_by(llm)) {
        for use_case in use_cases.iter_mut() {
            use_case.confidence = None;
        }
    }
    let entities: Vec<String> = Glossary::load(repo_root)
        .map(|g| g.entities().map(|(_, e)| e.name.clone()).collect())
        .unwrap_or_default();
    let pending: Vec<usize> = (0..use_cases.len())
        .filter(|&i| use_cases[i].confidence.is_none())
        .collect();
    // What the checks say of each pending use case; those that pass fully
    // are the ones a sample may send for a second opinion.
    let mut checked: Vec<(usize, Vec<StepCheck>)> = Vec::new();
    let mut passing: Vec<usize> = Vec::new();
    for &i in &pending {
        let (_, readable) = confidence_prompt(walked, &splitter, &use_cases[i]);
        if readable.is_empty() {
            use_cases[i].confidence = Some(ConfidenceScore::new(
                0.0,
                "no readable code cited by any step".to_string(),
            ));
            continue;
        }
        let checks: Vec<StepCheck> = use_cases[i]
            .steps
            .iter()
            .map(|s| check_step(walked, s, &entities))
            .collect();
        if checks.iter().all(|c| *c == StepCheck::Pass) {
            passing.push(i);
        }
        checked.push((i, checks));
    }
    let second_opinion = evenly_spaced(&passing, sample.unwrap_or(0));
    let mut chosen: Vec<usize> = Vec::new();
    let mut jobs_checks: Vec<Vec<StepCheck>> = Vec::new();
    for (i, mut checks) in checked {
        if second_opinion.contains(&i) {
            checks = vec![StepCheck::Doubtful("second opinion".to_string()); checks.len()];
        }
        if checks.iter().any(|c| matches!(c, StepCheck::Doubtful(_))) {
            chosen.push(i);
            jobs_checks.push(checks);
        } else {
            use_cases[i].confidence = Some(combine(&use_cases[i], &[], &checks));
        }
    }
    let checks_of: BTreeMap<usize, Vec<StepCheck>> =
        chosen.iter().copied().zip(jobs_checks).collect();

    let groups = request_groups(&chosen, use_cases);

    let mut progress = Progress::new("confidence", chosen.len());
    // The groups are checked `concurrency` at a time and their scores written in order.
    let splitter = &splitter;
    let jobs: Vec<Vec<(UseCase, Vec<StepCheck>)>> = groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|&i| (use_cases[i].clone(), checks_of[&i].clone()))
                .collect()
        })
        .collect();
    let mut results = stream::iter(jobs)
        .map(|owned| async move { score_group(walked, splitter, &owned, llm).await })
        .buffered(concurrency.max(1));
    let mut done = groups.iter();
    while let Some(scores) = results.next().await {
        let Some(group) = done.next() else { break };
        let label = use_cases[group[0]].name.clone();
        progress.start(&if group.len() > 1 {
            format!("{label} and {} more", group.len() - 1)
        } else {
            label
        });
        for (&i, score) in group.iter().zip(scores?) {
            use_cases[i].confidence = score;
        }
        progress.finish_many(group.len());
        save_scores(repo_root, use_cases, &mut prints, llm);
    }
    drop(results);
    for feature in features.iter_mut() {
        feature.confidence = feature_confidence(feature, use_cases);
    }
    save_use_cases(repo_root, use_cases)?;
    save_features(repo_root, features)?;
    prints.confidence_model = Some(scored_by(llm));
    prints.save(repo_root)
}

/// What the saved scores were given by: the model and the system prompt (ADR 0037), so another of
/// either scores them again.
fn scored_by(llm: &dyn LlmProvider) -> String {
    format!(
        "{} {}",
        llm.model(),
        prompt_part(&[CONFIDENCE_SYSTEM_PROMPT])
    )
}

/// The `chosen` use cases by request: a feature's use cases mostly cite the same code, so they
/// share one.
fn request_groups(chosen: &[usize], use_cases: &[UseCase]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &i in chosen {
        match groups.last_mut() {
            Some(group)
                if group.len() < MAX_USE_CASES_PER_REQUEST
                    && use_cases[group[0]].feature_slug == use_cases[i].feature_slug =>
            {
                group.push(i);
            }
            _ => groups.push(vec![i]),
        }
    }
    groups
}

/// Best-effort save after each answer: a run killed later loses the call in flight, nothing more.
/// The model goes with the scores, or the next run would take them for another model's.
fn save_scores(
    repo_root: &Path,
    use_cases: &[UseCase],
    prints: &mut Fingerprints,
    llm: &dyn LlmProvider,
) {
    prints.confidence_model = Some(scored_by(llm));
    warn_on_error(save_use_cases(repo_root, use_cases));
    warn_on_error(prints.save(repo_root));
}

/// Asks the model about the doubtful steps of the use cases of one feature:
/// one request for all of them when several, one per use case otherwise. A
/// use case the batched answer misses falls back to its own request.
async fn score_group(
    walked: &WalkedFiles,
    splitter: &Splitter,
    group: &[(UseCase, Vec<StepCheck>)],
    llm: &dyn LlmProvider,
) -> Result<Vec<Option<ConfidenceScore>>, PipelineError> {
    // Only the steps the checks left open are shown, with the code they cite.
    let asked: Vec<UseCase> = group.iter().map(|(u, c)| doubtful_part(u, c)).collect();
    let asked: Vec<&UseCase> = asked.iter().collect();
    let mut answers: BTreeMap<&str, Vec<RawVerdict>> = BTreeMap::new();
    let raw = if asked.len() > 1 {
        let what = format!("confidence of {} use cases", asked.len());
        complete_json::<RawBatchVerdicts>(
            llm,
            CONFIDENCE_SYSTEM_PROMPT,
            &batch_prompt(walked, splitter, &asked),
            &what,
        )
        .await?
    } else {
        None
    };
    if let Some(raw) = &raw {
        for item in &raw.use_cases {
            if let Some(u) = asked.iter().find(|u| u.slug == item.slug) {
                answers.insert(&u.slug, item.steps.clone());
            }
        }
    }
    let mut scores = Vec::new();
    for ((use_case, checks), part) in group.iter().zip(&asked) {
        scores.push(match answers.remove(use_case.slug.as_str()) {
            Some(steps) => Some(combine(use_case, &steps, checks)),
            None => score_use_case(walked, splitter, use_case, part, checks, llm).await?,
        });
    }
    Ok(scores)
}

/// The use case reduced to the steps the checks left doubtful.
fn doubtful_part(use_case: &UseCase, checks: &[StepCheck]) -> UseCase {
    let mut part = use_case.clone();
    part.steps = use_case
        .steps
        .iter()
        .zip(checks)
        .filter(|(_, c)| matches!(c, StepCheck::Doubtful(_)))
        .map(|(s, _)| s.clone())
        .collect();
    part
}

async fn score_use_case(
    walked: &WalkedFiles,
    splitter: &Splitter,
    use_case: &UseCase,
    part: &UseCase,
    checks: &[StepCheck],
    llm: &dyn LlmProvider,
) -> Result<Option<ConfidenceScore>, PipelineError> {
    let (prompt, _) = confidence_prompt(walked, splitter, part);
    let Some(raw) = complete_json::<RawVerdicts>(
        llm,
        CONFIDENCE_SYSTEM_PROMPT,
        &prompt,
        &format!("confidence of {}", use_case.slug),
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(combine(use_case, &raw.steps, checks)))
}

/// Turns the checks and the model's verdicts on the doubtful steps into the
/// use case score (mean of the step values, ungrounded steps capped) with a
/// rationale naming the weak steps.
fn combine(use_case: &UseCase, verdicts: &[RawVerdict], checks: &[StepCheck]) -> ConfidenceScore {
    let by_order: BTreeMap<u32, &RawVerdict> = verdicts.iter().map(|v| (v.order, v)).collect();
    let mut total = 0.0_f32;
    let mut weak: Vec<String> = Vec::new();
    for (step, check) in use_case.steps.iter().zip(checks) {
        let (value, note) = match check {
            StepCheck::Pass => (1.0, String::new()),
            StepCheck::Ungrounded => (UNGROUNDED_STEP_CAP, "cites no code".to_string()),
            StepCheck::Broken(why) => (0.0, why.clone()),
            StepCheck::Doubtful(_) => {
                let verdict = by_order.get(&step.order);
                (
                    verdict.map_or(0.0, |v| verdict_value(&v.verdict)),
                    verdict
                        .map_or("not assessed", |v| v.rationale.trim())
                        .to_string(),
                )
            }
        };
        total += value;
        if value < 1.0 {
            weak.push(if note.is_empty() {
                format!("step {}", step.order)
            } else {
                format!("step {}: {note}", step.order)
            });
        }
    }
    #[allow(clippy::cast_precision_loss)] // a use case has a handful of steps
    let value = total / use_case.steps.len().max(1) as f32;
    let rationale = if weak.is_empty() {
        "all steps supported by the cited code".to_string()
    } else {
        weak.join("; ")
    };
    ConfidenceScore::new(value, rationale)
}

/// Mean of the feature's use case scores; 0 without any use case, `None`
/// when it has some but none could be scored.
fn feature_confidence(feature: &Feature, use_cases: &[UseCase]) -> Option<ConfidenceScore> {
    let own: Vec<&UseCase> = use_cases
        .iter()
        .filter(|u| u.feature_slug == feature.slug)
        .collect();
    if own.is_empty() {
        return Some(ConfidenceScore::new(
            0.0,
            "no use case could be derived from its code".to_string(),
        ));
    }
    let scores: Vec<f32> = own
        .iter()
        .filter_map(|u| u.confidence.as_ref().map(|c| c.value))
        .collect();
    if scores.is_empty() {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let mean = scores.iter().sum::<f32>() / scores.len() as f32;
    let rationale = (scores.len() < own.len())
        .then(|| format!("{} of {} use case(s) scored", scores.len(), own.len()));
    Some(ConfidenceScore::new(mean, rationale))
}

/// The use case's steps, numbered.
fn steps_text(use_case: &UseCase) -> String {
    let mut text = String::new();
    for step in &use_case.steps {
        let _ = write!(
            text,
            "{}. [{}] {}",
            step.order,
            step.actor.name,
            step.label()
        );
        if let Some(detail) = step.detail() {
            let _ = write!(text, " — {detail}");
        }
        text.push('\n');
    }
    text
}

/// Paths cited by the steps of `use_cases`.
fn cited_paths<'a>(use_cases: &[&'a UseCase]) -> BTreeSet<&'a str> {
    use_cases
        .iter()
        .flat_map(|u| u.steps.iter())
        .flat_map(|s| s.source_refs.iter().map(|r| r.path.as_str()))
        .collect()
}

/// The lines of `path` that the steps of `use_cases` cite (a reference
/// without a range cites no line in particular).
fn cited_lines(use_cases: &[&UseCase], path: &str) -> Vec<(u32, u32)> {
    use_cases
        .iter()
        .flat_map(|u| u.steps.iter())
        .flat_map(|s| s.source_refs.iter())
        .filter(|r| r.path == path)
        .filter_map(|r| {
            let start = r.start_line.or(r.end_line)?;
            Some((start, r.end_line.unwrap_or(start).max(start)))
        })
        .collect()
}

/// Appends numbered excerpts of the files `use_cases` cite to `prompt`
/// within the prompt budget; a long file is shown around the cited lines.
/// Returns the files actually included.
fn append_code(
    walked: &WalkedFiles,
    splitter: &Splitter,
    use_cases: &[&UseCase],
    prompt: &mut String,
) -> BTreeSet<String> {
    let mut included = BTreeSet::new();
    let mut budget = MAX_CHARS_PER_PROMPT;
    for path in cited_paths(use_cases) {
        if budget == 0 {
            break;
        }
        let Ok(content) = walked.read(Path::new(path)) else {
            continue;
        };
        let focus = Focus {
            lines: cited_lines(use_cases, path),
            ..Focus::default()
        };
        let excerpt = splitter.excerpt(
            Path::new(path),
            &content,
            MAX_CHARS_PER_FILE.min(budget),
            &focus,
        );
        budget = budget.saturating_sub(excerpt.len());
        let _ = write!(prompt, "\n=== {path} ===\n{excerpt}\n");
        included.insert(path.to_string());
    }
    included
}

/// Prompt with the steps and the code they cite; also returns the files
/// actually included.
fn confidence_prompt(
    walked: &WalkedFiles,
    splitter: &Splitter,
    use_case: &UseCase,
) -> (String, BTreeSet<String>) {
    let mut prompt = format!(
        "Use case: {} — {}\n\nSteps:\n{}",
        use_case.name,
        use_case.description,
        steps_text(use_case)
    );
    let included = append_code(walked, splitter, &[use_case], &mut prompt);
    (prompt, included)
}

/// Prompt for several use cases of a feature: each one's steps, then the
/// code they cite, once. The answer is one verdict list per use case slug.
fn batch_prompt(walked: &WalkedFiles, splitter: &Splitter, use_cases: &[&UseCase]) -> String {
    let mut prompt = String::from(
        "Several use cases are checked against the same code. Reply with ONLY a JSON object \
         {\"use_cases\":[{\"slug\":\"...\",\"steps\":[{\"order\":1,\"verdict\":\"...\",\
         \"rationale\":\"...\"}]}]}, one entry per use case below, using its slug.\n",
    );
    for use_case in use_cases {
        let _ = write!(
            prompt,
            "\nUse case [{}]: {} — {}\n\nSteps:\n{}",
            use_case.slug,
            use_case.name,
            use_case.description,
            steps_text(use_case)
        );
    }
    append_code(walked, splitter, use_cases, &mut prompt);
    prompt
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // scores copied or exactly 0.0
mod tests;
