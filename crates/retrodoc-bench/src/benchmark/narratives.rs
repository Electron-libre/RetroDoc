//! The judge's rating of the narratives (`specs/benchmark-narrative-rating.md`): it asks, in batches,
//! whether each use case narrative of a run reads as business language.

use std::collections::BTreeSet;

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::error::PipelineError;
use crate::response::complete_json;
use crate::text::truncate_chars;

/// How many narratives of the use cases the judge reads, out of how many there are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrativeRating {
    pub total: usize,
    /// Narratives the judge gave a verdict on.
    pub rated: usize,
    /// Of those, the ones it found written in business language.
    pub business: usize,
    /// The rating of each batch, in the order they were sent; empty in a file saved before they
    /// were kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batches: Vec<BatchRating>,
}

/// The rating of one batch of narratives: a batch whose answer gave no usable verdict has none rated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchRating {
    /// Narratives sent in the batch.
    pub narratives: usize,
    pub rated: usize,
    pub business: usize,
}

impl NarrativeRating {
    /// Share of the rated narratives that read as business language; `None` when none was rated.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn business_share(&self) -> Option<f32> {
        (self.rated > 0).then(|| self.business as f32 / self.rated as f32)
    }

    /// The points by which the judge's share is off the run's `self_rated` business-language score
    /// (the mean of the use cases' own scores), when more than [`OFF_BOUND_POINTS`]: such a rating is
    /// to be rated again before it is read. `None` within the bound, or with no share or no score.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn off_by(&self, self_rated: Option<f32>) -> Option<u32> {
        let points = ((self.business_share()? - self_rated?).abs() * 100.0).round() as u32;
        (points > OFF_BOUND_POINTS).then_some(points)
    }
}

/// How many points the judge's share of narratives in business language may be off the run's
/// self-rated score: on the saved runs of 2026-11 a rating was within 18 points of it, and the bad
/// ones 46 points or more off.
pub const OFF_BOUND_POINTS: u32 = 25;

/// Narratives per call.
const BATCH: usize = 20;

/// Asks the judge, in batches, whether each narrative is written in business language (who does what
/// and why, in the application's words) rather than in code terms, and keeps the counts of each
/// batch. A batch it answers badly stays unrated, which the counts show.
///
/// # Errors
///
/// Returns an error if an LLM call fails.
pub async fn rate_narratives(
    llm: &dyn LlmProvider,
    narratives: &[String],
) -> Result<NarrativeRating, PipelineError> {
    let mut rating = NarrativeRating {
        total: narratives.len(),
        ..NarrativeRating::default()
    };
    for batch in narratives.chunks(BATCH) {
        let mut counts = BatchRating {
            narratives: batch.len(),
            ..BatchRating::default()
        };
        let user = batch
            .iter()
            .enumerate()
            .map(|(i, n)| format!("{}. {}", i + 1, truncate_chars(n, 600)))
            .collect::<Vec<_>>()
            .join("\n");
        let answer = complete_json::<RawRatings>(
            llm,
            RATE_SYSTEM_PROMPT,
            &user,
            "benchmark judge (narratives)",
        )
        .await?;
        let mut seen = BTreeSet::new();
        for verdict in answer.map(|a| a.ratings).unwrap_or_default() {
            if verdict.id == 0 || verdict.id > batch.len() || !seen.insert(verdict.id) {
                continue;
            }
            counts.rated += 1;
            counts.business += usize::from(verdict.business);
        }
        rating.rated += counts.rated;
        rating.business += counts.business;
        rating.batches.push(counts);
    }
    Ok(rating)
}

const RATE_SYSTEM_PROMPT: &str = "You are reading short descriptions of what users do with a \
software application. For each numbered one, say whether it is written in business language: who \
does what and why, in the words of the application's users, with no class, function, table, \
route or framework terms. The names of the application's own business objects (a Bookmark, an \
API token, a Tag), a mention of an API client or of an exported file, and the plain description of \
what the screen shows are business language; technical mechanisms (queues, migrations, caches, \
database or framework internals) and developer tooling (build, test, deployment scripts) are not. \
Reply with ONLY a single JSON object, no prose and no Markdown code \
fence, matching this shape: {\"ratings\":[{\"id\":1,\"business\":true}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawRatings {
    #[serde(default)]
    ratings: Vec<RawRating>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawRating {
    id: usize,
    business: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeLlm;

    #[tokio::test]
    async fn rates_the_narratives_it_is_given_a_verdict_for() {
        let llm = FakeLlm::answering(
            r#"{"ratings":[{"id":1,"business":true},{"id":2,"business":false},{"id":7,"business":true},{"id":1,"business":false}]}"#,
        );
        let narratives: Vec<String> = ["A clerk invoices.", "Calls the service.", "Unrated one."]
            .iter()
            .map(ToString::to_string)
            .collect();

        let rating = rate_narratives(&llm, &narratives).await.unwrap();

        assert_eq!(
            rating,
            NarrativeRating {
                total: 3,
                rated: 2,
                business: 1,
                batches: vec![BatchRating {
                    narratives: 3,
                    rated: 2,
                    business: 1
                }],
            },
            "id 7 doesn't exist, the second verdict for id 1 is ignored"
        );
        assert_eq!(rating.business_share(), Some(0.5));
        assert_eq!(llm.calls(), 1);
    }

    // spec: BNR-4 (internal: the batches are read from the rating built with a fake judge)
    #[tokio::test]
    async fn narratives_go_by_batches_and_a_bad_batch_stays_unrated() {
        let llm = FakeLlm::sequence(&[r#"{"ratings":[{"id":1,"business":true}]}"#, "garbage"]);
        let narratives: Vec<String> = (0..25).map(|i| format!("narrative {i}")).collect();

        let rating = rate_narratives(&llm, &narratives).await.unwrap();

        assert_eq!((rating.total, rating.rated, rating.business), (25, 1, 1));
        assert_eq!(
            rating.batches,
            [
                BatchRating {
                    narratives: 20,
                    rated: 1,
                    business: 1
                },
                BatchRating {
                    narratives: 5,
                    rated: 0,
                    business: 0
                }
            ],
            "the unparseable batch is kept with none rated"
        );
        // Batch 1 answered once; batch 2 was unparseable twice (one retry).
        assert_eq!(llm.calls(), 3);
        assert!(llm.prompts()[0].contains("narrative 19"));
        assert!(!llm.prompts()[0].contains("narrative 20"));
    }

    #[test]
    fn no_rated_narrative_is_no_share() {
        assert_eq!(NarrativeRating::default().business_share(), None);
    }

    // spec: BNR-4 (internal: the file is parsed from a string, as `judge.yaml` holds it)
    #[test]
    fn a_rating_saved_before_the_batches_loads_with_none() {
        let rating: NarrativeRating =
            serde_yaml::from_str("total: 73\nrated: 73\nbusiness: 22\n").unwrap();

        assert_eq!((rating.total, rating.rated, rating.business), (73, 73, 22));
        assert_eq!(rating.batches, []);
    }

    fn rated(business: usize, rated: usize) -> NarrativeRating {
        NarrativeRating {
            total: rated,
            rated,
            business,
            ..NarrativeRating::default()
        }
    }

    // spec: BNR-2 (internal: the bound is read on ratings built in memory)
    #[test]
    fn a_rating_more_than_the_bound_off_the_self_rated_score_is_off() {
        assert_eq!(rated(73, 100).off_by(Some(0.98)), None, "25 points");
        assert_eq!(rated(72, 100).off_by(Some(0.98)), Some(26));
        assert_eq!(rated(22, 73).off_by(Some(0.976_712_3)), Some(68));
        assert_eq!(rated(100, 100).off_by(Some(0.5)), Some(50), "above it too");
        assert_eq!(rated(0, 0).off_by(Some(0.98)), None, "nothing rated");
        assert_eq!(rated(22, 73).off_by(None), None, "no self-rated score");
    }
}
