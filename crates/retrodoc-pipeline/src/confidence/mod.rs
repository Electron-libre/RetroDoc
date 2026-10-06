//! Confidence pass (PLAN.md §2 step 7, roadmap phase 5): a cross-check of
//! each use case against the code it cites. One LLM call per use case gets
//! the steps and the excerpts of the cited files and must give a verdict per
//! step (`supported`, `partial`, `unsupported`), which is turned into a
//! [`ConfidenceScore`].
//!
//! Scoring is deliberately not left to the model alone:
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

use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::chunks::{Focus, Splitter};
use crate::error::PipelineError;
use crate::features::save_features;
use crate::progress::Progress;
use crate::response::complete_json;
use crate::roles::load_splitter;
use crate::use_cases::save_use_cases;

/// Highest score a step without any source reference can get.
const UNGROUNDED_STEP_CAP: f32 = 0.25;

/// Use cases of a feature judged in one request.
const MAX_USE_CASES_PER_REQUEST: usize = 5;

const MAX_CHARS_PER_FILE: usize = 4000;
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

#[derive(Debug, Deserialize)]
struct RawVerdicts {
    #[serde(default)]
    steps: Vec<RawVerdict>,
}

#[derive(Debug, Deserialize)]
struct RawBatchVerdicts {
    #[serde(default)]
    use_cases: Vec<RawUseCaseVerdicts>,
}

#[derive(Debug, Deserialize)]
struct RawUseCaseVerdicts {
    slug: String,
    #[serde(default)]
    steps: Vec<RawVerdict>,
}

#[derive(Debug, Clone, Deserialize)]
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

/// Scores every use case not scored yet (reused use cases keep their score
/// from the last run) — or only `sample` of them, evenly spread over the
/// pending ones, the others staying unscored (`None`) — then every feature, and persists both artifacts
/// (`use-cases.yaml`, `features.yaml`) with their `confidence` filled in.
///
/// # Errors
///
/// Returns an error if an LLM call fails or an artifact can't be saved.
pub async fn score_confidence(
    repo_root: &Path,
    features: &mut [Feature],
    use_cases: &mut [UseCase],
    llm: &dyn LlmProvider,
    sample: Option<usize>,
) -> Result<(), PipelineError> {
    let splitter = load_splitter(repo_root);
    let pending: Vec<usize> = (0..use_cases.len())
        .filter(|&i| use_cases[i].confidence.is_none())
        .collect();
    let chosen_count = sample.map_or(pending.len(), |n| n.min(pending.len()));
    // Picks `chosen_count` of the pending ranks, evenly spaced.
    let total = pending.len().max(1);
    let chosen: Vec<usize> = pending
        .iter()
        .enumerate()
        .filter(|(rank, _)| rank * chosen_count / total != (rank + 1) * chosen_count / total)
        .map(|(_, &i)| i)
        .collect();

    // A feature's use cases mostly cite the same code: they share a request.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &i in &chosen {
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

    let mut progress = Progress::new("confidence", chosen.len());
    for group in groups {
        let label = use_cases[group[0]].name.clone();
        progress.start(&if group.len() > 1 {
            format!("{label} and {} more", group.len() - 1)
        } else {
            label
        });
        let members: Vec<&UseCase> = group.iter().map(|&i| &use_cases[i]).collect();
        let scores = score_group(repo_root, &splitter, &members, llm).await?;
        for (&i, score) in group.iter().zip(scores) {
            use_cases[i].confidence = score;
        }
        progress.finish_many(group.len());
    }
    for feature in features.iter_mut() {
        feature.confidence = feature_confidence(feature, use_cases);
    }
    save_use_cases(repo_root, use_cases)?;
    save_features(repo_root, features)
}

/// Scores use cases of one feature: one request for all of them when
/// several have readable code, one per use case otherwise. A use case the
/// batched answer misses falls back to its own request.
async fn score_group(
    repo_root: &Path,
    splitter: &Splitter,
    group: &[&UseCase],
    llm: &dyn LlmProvider,
) -> Result<Vec<Option<ConfidenceScore>>, PipelineError> {
    let live: Vec<&UseCase> = group
        .iter()
        .copied()
        .filter(|u| !confidence_prompt(repo_root, splitter, u).1.is_empty())
        .collect();
    let mut answers: BTreeMap<&str, Vec<RawVerdict>> = BTreeMap::new();
    let raw = if live.len() > 1 {
        let what = format!("confidence of {} use cases", live.len());
        complete_json::<RawBatchVerdicts>(
            llm,
            CONFIDENCE_SYSTEM_PROMPT,
            &batch_prompt(repo_root, splitter, &live),
            &what,
        )
        .await?
    } else {
        None
    };
    if let Some(raw) = &raw {
        for item in &raw.use_cases {
            if let Some(u) = live.iter().find(|u| u.slug == item.slug) {
                answers.insert(&u.slug, item.steps.clone());
            }
        }
    }
    let mut scores = Vec::new();
    for use_case in group {
        scores.push(match answers.remove(use_case.slug.as_str()) {
            Some(steps) => Some(combine(use_case, &steps)),
            None => score_use_case(repo_root, splitter, use_case, llm).await?,
        });
    }
    Ok(scores)
}

async fn score_use_case(
    repo_root: &Path,
    splitter: &Splitter,
    use_case: &UseCase,
    llm: &dyn LlmProvider,
) -> Result<Option<ConfidenceScore>, PipelineError> {
    let (prompt, readable) = confidence_prompt(repo_root, splitter, use_case);
    if readable.is_empty() {
        return Ok(Some(ConfidenceScore::new(
            0.0,
            "no readable code cited by any step".to_string(),
        )));
    }
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
    Ok(Some(combine(use_case, &raw.steps)))
}

/// Turns the per-step verdicts into the use case score (mean of the step
/// values, ungrounded steps capped) with a rationale naming the weak steps.
fn combine(use_case: &UseCase, verdicts: &[RawVerdict]) -> ConfidenceScore {
    let by_order: BTreeMap<u32, &RawVerdict> = verdicts.iter().map(|v| (v.order, v)).collect();
    let mut total = 0.0_f32;
    let mut weak: Vec<String> = Vec::new();
    for step in &use_case.steps {
        let verdict = by_order.get(&step.order);
        let mut value = verdict.map_or(0.0, |v| verdict_value(&v.verdict));
        let mut note = verdict
            .map_or("not assessed", |v| v.rationale.trim())
            .to_string();
        if step.source_refs.is_empty() && value > UNGROUNDED_STEP_CAP {
            value = UNGROUNDED_STEP_CAP;
            note = "cites no code".to_string();
        }
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
        let _ = writeln!(
            text,
            "{}. [{}] {} — {}",
            step.order, step.actor.name, step.action, step.description
        );
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
    repo_root: &Path,
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
        let Ok(bytes) = std::fs::read(repo_root.join(path)) else {
            continue;
        };
        let focus = Focus {
            lines: cited_lines(use_cases, path),
            ..Focus::default()
        };
        let excerpt = splitter.excerpt(
            Path::new(path),
            &String::from_utf8_lossy(&bytes),
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
    repo_root: &Path,
    splitter: &Splitter,
    use_case: &UseCase,
) -> (String, BTreeSet<String>) {
    let mut prompt = format!(
        "Use case: {} — {}\n\nSteps:\n{}",
        use_case.name,
        use_case.description,
        steps_text(use_case)
    );
    let included = append_code(repo_root, splitter, &[use_case], &mut prompt);
    (prompt, included)
}

/// Prompt for several use cases of a feature: each one's steps, then the
/// code they cite, once. The answer is one verdict list per use case slug.
fn batch_prompt(repo_root: &Path, splitter: &Splitter, use_cases: &[&UseCase]) -> String {
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
    append_code(repo_root, splitter, use_cases, &mut prompt);
    prompt
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // scores copied or exactly 0.0
mod tests;
