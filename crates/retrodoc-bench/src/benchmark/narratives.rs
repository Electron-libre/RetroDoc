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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchRating {
    /// Narratives sent in the batch.
    pub narratives: usize,
    /// The counts kept: those of the batch's only rating, or the middle one of a batch rated again.
    pub rated: usize,
    pub business: usize,
    /// Each rating of a batch rated again, the first included, in the order they were made; empty for
    /// a batch rated once and in a file saved before they were kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ratings: Vec<Verdicts>,
}

/// What one answer of the judge gave for a batch: the narratives it rated, the business ones among them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdicts {
    pub rated: usize,
    pub business: usize,
}

impl Verdicts {
    #[allow(clippy::cast_precision_loss)]
    fn share(self) -> Option<f32> {
        (self.rated > 0).then(|| self.business as f32 / self.rated as f32)
    }
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

/// How many points a batch's share may be below the median of the other batches' before it is rated
/// again: on the saved ratings of 2026-11 a flipped batch read 82 points or more below; a batch of
/// technical use cases reads up to 70 points below, and its middle rating keeps it low.
const RERATE_BOUND_POINTS: u32 = 50;

/// Ratings a flipped batch gets after its first, so that the middle one outvotes a single flip.
const RERATINGS: usize = 2;

/// Asks the judge, in batches, whether each narrative is written in business language (who does what
/// and why, in the application's words) rather than in code terms, and keeps the counts of each
/// batch. A batch it answers badly stays unrated, which the counts show; a batch far below the others
/// is rated again and keeps its middle rating (`specs/benchmark-narrative-rating.md`, BNR-6).
///
/// # Errors
///
/// Returns an error if an LLM call fails.
pub async fn rate_narratives(
    llm: &dyn LlmProvider,
    narratives: &[String],
) -> Result<NarrativeRating, PipelineError> {
    let chunks: Vec<&[String]> = narratives.chunks(BATCH).collect();
    let mut firsts = Vec::with_capacity(chunks.len());
    for batch in &chunks {
        firsts.push(rate_batch(llm, batch).await?);
    }
    let mut rating = NarrativeRating {
        total: narratives.len(),
        ..NarrativeRating::default()
    };
    for (i, batch) in chunks.iter().enumerate() {
        let mut counts = BatchRating {
            narratives: batch.len(),
            ..BatchRating::default()
        };
        let mut kept = firsts[i];
        if is_flipped(&firsts, i) {
            counts.ratings.push(firsts[i]);
            for _ in 0..RERATINGS {
                counts.ratings.push(rate_batch(llm, batch).await?);
            }
            kept = middle(&counts.ratings);
        }
        counts.rated = kept.rated;
        counts.business = kept.business;
        rating.rated += kept.rated;
        rating.business += kept.business;
        rating.batches.push(counts);
    }
    Ok(rating)
}

/// One answer of the judge on a batch; an answer with no usable verdict rates none.
async fn rate_batch(llm: &dyn LlmProvider, batch: &[String]) -> Result<Verdicts, PipelineError> {
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
    let mut verdicts = Verdicts::default();
    let mut seen = BTreeSet::new();
    for verdict in answer.map(|a| a.ratings).unwrap_or_default() {
        if verdict.id == 0 || verdict.id > batch.len() || !seen.insert(verdict.id) {
            continue;
        }
        verdicts.rated += 1;
        verdicts.business += usize::from(verdict.business);
    }
    Ok(verdicts)
}

/// Whether the first rating of batch `i` is more than [`RERATE_BOUND_POINTS`] below the median of
/// the other batches' first ratings; a batch with no verdict, or with no other batch rated, is not.
#[allow(clippy::cast_possible_truncation)]
fn is_flipped(firsts: &[Verdicts], i: usize) -> bool {
    let Some(share) = firsts[i].share() else {
        return false;
    };
    let mut others: Vec<f32> = firsts
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != i)
        .filter_map(|(_, v)| v.share())
        .collect();
    if others.is_empty() {
        return false;
    }
    others.sort_by(f32::total_cmp);
    let half = others.len() / 2;
    let median = if others.len().is_multiple_of(2) {
        f32::midpoint(others[half - 1], others[half])
    } else {
        others[half]
    };
    ((median - share) * 100.0).round() as i64 > i64::from(RERATE_BOUND_POINTS)
}

/// The rating whose share is the middle one of those that gave a verdict (the higher of two, the
/// earlier between equal shares); none rated when none gave one.
fn middle(ratings: &[Verdicts]) -> Verdicts {
    let by_share = |a: &Verdicts, b: &Verdicts| (a.business * b.rated).cmp(&(b.business * a.rated));
    let mut usable: Vec<&Verdicts> = ratings.iter().filter(|v| v.rated > 0).collect();
    usable.sort_by(|a, b| by_share(a, b));
    let Some(&kept) = usable.get(usable.len() / 2) else {
        return Verdicts::default();
    };
    // The earliest rating of that share.
    *ratings
        .iter()
        .find(|v| v.rated > 0 && by_share(v, kept).is_eq())
        .unwrap_or(kept)
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
                    business: 1,
                    ..BatchRating::default()
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
                    business: 1,
                    ..BatchRating::default()
                },
                BatchRating {
                    narratives: 5,
                    rated: 0,
                    business: 0,
                    ..BatchRating::default()
                }
            ],
            "the unparseable batch is kept with none rated"
        );
        // Batch 1 answered once; batch 2 was unparseable twice (one retry).
        assert_eq!(llm.calls(), 3);
        assert!(llm.prompts()[0].contains("narrative 19"));
        assert!(!llm.prompts()[0].contains("narrative 20"));
    }

    /// The judge's answer to a batch of `narratives`, the first `business` of them in business language.
    fn answer(narratives: usize, business: usize) -> String {
        let ratings: Vec<String> = (1..=narratives)
            .map(|id| format!(r#"{{"id":{id},"business":{}}}"#, id <= business))
            .collect();
        format!(r#"{{"ratings":[{}]}}"#, ratings.join(","))
    }

    async fn rate(answers: &[String], narratives: usize) -> (NarrativeRating, usize) {
        let answers: Vec<&str> = answers.iter().map(String::as_str).collect();
        let llm = FakeLlm::sequence(&answers);
        let narratives: Vec<String> = (0..narratives).map(|i| format!("narrative {i}")).collect();
        let rating = rate_narratives(&llm, &narratives).await.unwrap();
        (rating, llm.calls())
    }

    fn verdicts(rated: usize, business: usize) -> Verdicts {
        Verdicts { rated, business }
    }

    // spec: BNR-H3, BNR-6, BNR-7, BNR-9 (internal: the judge is a fake answering each batch in turn)
    #[tokio::test]
    async fn a_batch_far_below_the_others_is_rated_twice_more_and_keeps_the_middle_rating() {
        let (rating, calls) = rate(
            &[
                answer(20, 19),
                answer(20, 0),
                answer(20, 17),
                answer(13, 8),
                answer(20, 17),
                answer(20, 15),
            ],
            73,
        )
        .await;

        assert_eq!(calls, 6);
        assert_eq!((rating.rated, rating.business), (73, 59));
        let second = &rating.batches[1];
        assert_eq!(
            (second.narratives, second.rated, second.business),
            (20, 20, 15)
        );
        assert_eq!(
            second.ratings,
            [verdicts(20, 0), verdicts(20, 17), verdicts(20, 15)]
        );
        assert!(
            [0, 2, 3]
                .iter()
                .all(|&i| rating.batches[i].ratings.is_empty()),
            "a batch rated once keeps no ratings: {rating:?}"
        );
    }

    // spec: BNR-6 (internal: the judge is a fake answering each batch in turn)
    #[tokio::test]
    async fn a_batch_up_to_fifty_points_below_the_others_is_rated_once() {
        let at_bound = rate(&[answer(20, 18), answer(20, 20), answer(20, 9)], 60).await;
        assert_eq!(at_bound.1, 3, "45% is 50 points below 95%");

        let past_it = rate(
            &[
                answer(20, 18),
                answer(20, 20),
                answer(9, 4),
                answer(9, 9),
                answer(9, 9),
            ],
            49,
        )
        .await;
        assert_eq!(past_it.1, 5, "44% is 51 points below 95%");
        assert_eq!(past_it.0.batches[2].business, 9);
    }

    // spec: BNR-6 (internal: the judge is a fake answering each batch in turn)
    #[tokio::test]
    async fn a_single_batch_or_an_unusable_one_is_never_rated_again() {
        let single = rate(&[answer(5, 0)], 5).await;
        assert_eq!(single.1, 1);

        let unusable = rate(
            &[
                answer(20, 20),
                "garbage".to_string(),
                "garbage".to_string(),
                answer(20, 20),
            ],
            60,
        )
        .await;
        assert_eq!(
            unusable.1, 4,
            "the garbage answer is retried once, never rated again"
        );
        assert_eq!(unusable.0.batches[1].rated, 0);
        assert_eq!(unusable.0.batches[1].ratings, []);
    }

    // spec: BNR-7 (internal: the judge is a fake answering each batch in turn)
    #[tokio::test]
    async fn a_re_rating_with_no_verdict_is_saved_and_the_higher_of_the_two_others_kept() {
        let (rating, _) = rate(
            &[
                answer(20, 20),
                answer(20, 0),
                answer(20, 20),
                "garbage".to_string(),
                "garbage".to_string(),
                answer(20, 16),
            ],
            60,
        )
        .await;

        let second = &rating.batches[1];
        assert_eq!((second.rated, second.business), (20, 16));
        assert_eq!(
            second.ratings,
            [verdicts(20, 0), verdicts(0, 0), verdicts(20, 16)]
        );
        assert_eq!((rating.rated, rating.business), (60, 56));
    }

    // spec: BNR-7 (internal: the judge is a fake answering each batch in turn)
    #[tokio::test]
    async fn a_technical_batch_stays_technical() {
        let (rating, _) = rate(
            &[
                answer(20, 20),
                answer(20, 0),
                answer(20, 20),
                answer(20, 1),
                answer(20, 2),
            ],
            60,
        )
        .await;

        assert_eq!(rating.batches[1].business, 1);
    }

    #[test]
    fn between_equal_shares_the_earlier_rating_is_kept() {
        let kept = middle(&[verdicts(20, 10), verdicts(10, 5)]);
        assert_eq!(kept, verdicts(20, 10));
        let kept = middle(&[verdicts(20, 0), verdicts(10, 5), verdicts(20, 10)]);
        assert_eq!(kept, verdicts(10, 5));
    }

    // spec: BNR-9 (internal: the file is parsed from a string, as `judge.yaml` holds it)
    #[test]
    fn a_batch_saved_before_its_ratings_loads_with_none() {
        let batch: BatchRating =
            serde_yaml::from_str("narratives: 20\nrated: 20\nbusiness: 3\n").unwrap();

        assert_eq!(batch.ratings, []);
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
