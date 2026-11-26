//! The judge's rating of the narratives (`specs/benchmark-narrative-rating.md`): it asks, in batches,
//! whether each use case narrative of a run reads as business language.

use std::collections::BTreeSet;

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::error::PipelineError;
use crate::response::complete_json;
use crate::text::truncate_chars;

/// How many narratives of the use cases the judge reads, out of how many there are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrativeRating {
    pub total: usize,
    /// Narratives the judge gave a verdict on.
    pub rated: usize,
    /// Of those, the ones it found written in business language.
    pub business: usize,
}

impl NarrativeRating {
    /// Share of the rated narratives that read as business language; `None` when none was rated.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn business_share(&self) -> Option<f32> {
        (self.rated > 0).then(|| self.business as f32 / self.rated as f32)
    }
}

/// Narratives per call.
const BATCH: usize = 20;

/// Asks the judge, in batches, whether each narrative is written in business language (who does what
/// and why, in the application's words) rather than in code terms. A batch it answers badly stays
/// unrated, which the counts show.
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
        let user = batch
            .iter()
            .enumerate()
            .map(|(i, n)| format!("{}. {}", i + 1, truncate_chars(n, 600)))
            .collect::<Vec<_>>()
            .join("\n");
        let Some(answer) = complete_json::<RawRatings>(
            llm,
            RATE_SYSTEM_PROMPT,
            &user,
            "benchmark judge (narratives)",
        )
        .await?
        else {
            continue;
        };
        let mut seen = BTreeSet::new();
        for verdict in answer.ratings {
            if verdict.id == 0 || verdict.id > batch.len() || !seen.insert(verdict.id) {
                continue;
            }
            rating.rated += 1;
            rating.business += usize::from(verdict.business);
        }
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
                business: 1
            },
            "id 7 doesn't exist, the second verdict for id 1 is ignored"
        );
        assert_eq!(rating.business_share(), Some(0.5));
        assert_eq!(llm.calls(), 1);
    }

    #[tokio::test]
    async fn narratives_go_by_batches_and_a_bad_batch_stays_unrated() {
        let llm = FakeLlm::sequence(&[r#"{"ratings":[{"id":1,"business":true}]}"#, "garbage"]);
        let narratives: Vec<String> = (0..25).map(|i| format!("narrative {i}")).collect();

        let rating = rate_narratives(&llm, &narratives).await.unwrap();

        assert_eq!(
            rating,
            NarrativeRating {
                total: 25,
                rated: 1,
                business: 1
            }
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
}
