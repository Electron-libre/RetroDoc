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

use crate::error::PipelineError;
use crate::features::save_features;
use crate::response::complete_json;
use crate::use_cases::{numbered_excerpt, save_use_cases};

/// Highest score a step without any source reference can get.
const UNGROUNDED_STEP_CAP: f32 = 0.25;

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

/// Scores every use case, then every feature, and persists both artifacts
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
) -> Result<(), PipelineError> {
    for use_case in use_cases.iter_mut() {
        use_case.confidence = score_use_case(repo_root, use_case, llm).await?;
    }
    for feature in features.iter_mut() {
        feature.confidence = feature_confidence(feature, use_cases);
    }
    save_use_cases(repo_root, use_cases)?;
    save_features(repo_root, features)
}

async fn score_use_case(
    repo_root: &Path,
    use_case: &UseCase,
    llm: &dyn LlmProvider,
) -> Result<Option<ConfidenceScore>, PipelineError> {
    let (prompt, readable) = confidence_prompt(repo_root, use_case);
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

/// Prompt with the steps and the code they cite; also returns the files
/// actually included.
fn confidence_prompt(repo_root: &Path, use_case: &UseCase) -> (String, BTreeSet<String>) {
    let mut prompt = format!(
        "Use case: {} — {}\n\nSteps:\n",
        use_case.name, use_case.description
    );
    for step in &use_case.steps {
        let _ = writeln!(
            prompt,
            "{}. [{}] {} — {}",
            step.order, step.actor.name, step.action, step.description
        );
    }
    let cited: BTreeSet<&str> = use_case
        .steps
        .iter()
        .flat_map(|s| s.source_refs.iter().map(|r| r.path.as_str()))
        .collect();
    let mut included = BTreeSet::new();
    let mut budget = MAX_CHARS_PER_PROMPT;
    for path in cited {
        if budget == 0 {
            break;
        }
        let Ok(bytes) = std::fs::read(repo_root.join(path)) else {
            continue;
        };
        let excerpt = numbered_excerpt(
            &String::from_utf8_lossy(&bytes),
            MAX_CHARS_PER_FILE.min(budget),
        );
        budget = budget.saturating_sub(excerpt.len());
        let _ = write!(prompt, "\n=== {path} ===\n{excerpt}\n");
        included.insert(path.to_string());
    }
    (prompt, included)
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // scores copied or exactly 0.0
mod tests {
    use super::*;

    use async_trait::async_trait;
    use retrodoc_core::model::{Actor, ActorKind, SourceRef, Step};
    use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError};

    struct CannedProvider(&'static str);

    #[async_trait]
    impl LlmProvider for CannedProvider {
        async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            Ok(CompletionResponse {
                content: self.0.to_string(),
                model: "test-model".to_string(),
            })
        }
    }

    fn step(order: u32, path: Option<&str>) -> Step {
        Step {
            order,
            description: "d".to_string(),
            actor: Actor {
                name: "API".to_string(),
                kind: ActorKind::System,
            },
            action: "does".to_string(),
            source_refs: path
                .map(|p| SourceRef {
                    path: p.to_string(),
                    start_line: None,
                    end_line: None,
                })
                .into_iter()
                .collect(),
        }
    }

    fn use_case(feature: &str, steps: Vec<Step>) -> UseCase {
        UseCase {
            slug: "u".to_string(),
            feature_slug: feature.to_string(),
            name: "U".to_string(),
            description: "d".to_string(),
            steps,
            diagram_mermaid: None,
            confidence: None,
        }
    }

    fn feature(slug: &str) -> Feature {
        Feature {
            slug: slug.to_string(),
            domain_slug: "billing".to_string(),
            sub_domain_slug: None,
            name: slug.to_string(),
            description: "d".to_string(),
            source_paths: vec![],
            confidence: None,
        }
    }

    #[tokio::test]
    async fn scores_steps_caps_ungrounded_ones_and_aggregates_to_the_feature() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        let provider = CannedProvider(
            r#"{"steps":[
              {"order":1,"verdict":"supported"},
              {"order":2,"verdict":"supported"},
              {"order":3,"verdict":"unsupported","rationale":"nothing about emails"}]}"#,
        );
        let mut features = vec![feature("f"), feature("empty")];
        let mut use_cases = vec![use_case(
            "f",
            vec![step(1, Some("a.rs")), step(2, None), step(3, Some("a.rs"))],
        )];

        score_confidence(dir.path(), &mut features, &mut use_cases, &provider)
            .await
            .unwrap();

        // (1.0 + 0.25 + 0.0) / 3
        let uc = use_cases[0].confidence.as_ref().unwrap();
        assert!((uc.value - 1.25 / 3.0).abs() < 1e-6);
        let why = uc.rationale.as_deref().unwrap();
        assert!(why.contains("step 2: cites no code"));
        assert!(why.contains("step 3: nothing about emails"));
        assert_eq!(features[0].confidence.as_ref().unwrap().value, uc.value);
        assert_eq!(features[1].confidence.as_ref().unwrap().value, 0.0);

        let saved = crate::load_use_cases(dir.path()).unwrap();
        assert!(saved[0].confidence.is_some());
        assert!(crate::load_features(dir.path()).unwrap()[0]
            .confidence
            .is_some());
    }

    #[tokio::test]
    async fn no_readable_code_scores_zero_and_bad_answer_stays_unscored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        let mut features = vec![feature("f")];
        let mut use_cases = vec![
            use_case("f", vec![step(1, None)]),
            use_case("f", vec![step(1, Some("a.rs"))]),
        ];

        score_confidence(
            dir.path(),
            &mut features,
            &mut use_cases,
            &CannedProvider("nope"),
        )
        .await
        .unwrap();

        assert_eq!(use_cases[0].confidence.as_ref().unwrap().value, 0.0);
        assert!(use_cases[1].confidence.is_none());
        // Feature mean only over the scored use case.
        let fc = features[0].confidence.as_ref().unwrap();
        assert_eq!(fc.value, 0.0);
        assert_eq!(fc.rationale.as_deref(), Some("1 of 2 use case(s) scored"));
    }
}
