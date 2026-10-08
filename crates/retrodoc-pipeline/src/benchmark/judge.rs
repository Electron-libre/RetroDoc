//! The LLM judge of the benchmark. It reads what the deterministic matching left out:
//!
//! - it proposes pairs between the generated domains/features that match no reference item and the
//!   reference items that no generated item covers (a pair is a proposal: the user reads it, fixes it
//!   and freezes it in `matches.yaml`, which always wins);
//! - it rates whether each use case narrative reads as business language, in batches.
//!
//! The judge shares the biases of the model that generated the docs, so its verdicts are saved apart
//! (`judge.yaml`) and the figures built on them are reported next to the ones that are not.

use std::collections::BTreeSet;
use std::path::Path;

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use super::matching::{compare, Matches, Pair};
use super::reference::Reference;
use crate::domains::DomainMap;
use crate::error::PipelineError;
use crate::features::load_features;
use crate::naming::normalize;
use crate::repo_map::truncate_chars;
use crate::response::complete_json;
use crate::use_cases::load_use_cases;

/// A name with what it means, as shown to the judge.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub description: String,
}

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

/// What the judge said about one run: written to `judge.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Judgement {
    pub domains: Vec<Pair>,
    pub features: Vec<Pair>,
    pub narratives: NarrativeRating,
}

impl Judgement {
    /// Where the judgement of the run in `repo_root` is kept, apart from the generated artifacts.
    #[must_use]
    pub fn path(repo_root: &Path) -> std::path::PathBuf {
        repo_root.join(".retrodoc/benchmark/judge.yaml")
    }

    /// Writes `.retrodoc/benchmark/judge.yaml`, for the user to read and correct.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be written.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        crate::artifact::save_yaml(&Self::path(repo_root), self)
    }
}

impl Matches {
    /// The hand-written pairs followed by the judge's proposals (the first pair for a generated name
    /// wins, so the manual ones keep priority).
    #[must_use]
    pub fn with_proposals(&self, judgement: &Judgement) -> Matches {
        Matches {
            domains: [self.domains.as_slice(), judgement.domains.as_slice()].concat(),
            features: [self.features.as_slice(), judgement.features.as_slice()].concat(),
        }
    }
}

/// Asks the judge to pair `generated` items with `reference` ones that stand for the same business
/// capability. Pairs naming an item that wasn't offered are dropped, and so are repeated generated
/// names. No call when either side is empty.
///
/// # Errors
///
/// Returns an error if the LLM call fails.
pub async fn propose_matches(
    llm: &dyn LlmProvider,
    what: &str,
    generated: &[Item],
    reference: &[Item],
) -> Result<Vec<Pair>, PipelineError> {
    if generated.is_empty() || reference.is_empty() {
        return Ok(Vec::new());
    }
    let user = format!(
        "Generated {what}:\n{}\nReference {what}:\n{}",
        listing(generated),
        listing(reference)
    );
    let Some(answer) =
        complete_json::<RawMatches>(llm, MATCH_SYSTEM_PROMPT, &user, "benchmark judge (matches)")
            .await?
    else {
        return Ok(Vec::new());
    };
    let known = |items: &[Item], name: &str| {
        let wanted = normalize(name);
        items.iter().any(|i| normalize(&i.name) == wanted)
    };
    let mut pairs: Vec<Pair> = Vec::new();
    for raw in answer.matches {
        let repeated = pairs
            .iter()
            .any(|p| normalize(&p.generated) == normalize(&raw.generated));
        if known(generated, &raw.generated) && known(reference, &raw.reference) && !repeated {
            pairs.push(raw);
        } else {
            tracing::debug!(
                generated = %raw.generated,
                reference = %raw.reference,
                "benchmark judge: proposal dropped"
            );
        }
    }
    Ok(pairs)
}

const MATCH_SYSTEM_PROMPT: &str = "You are checking generated software documentation against a \
hand-written reference. You get the generated items and the reference items, each with a \
description. Pair a generated item with a reference item only when both describe the same \
business capability, even if their names differ. Leave out what has no counterpart: an \
unpaired item is a normal answer. A generated item is paired with at most one reference item; \
several generated items may share one reference item. Copy the names exactly as written. Reply \
with ONLY a single JSON object, no prose and no Markdown code fence, matching this shape: \
{\"matches\":[{\"generated\":\"...\",\"reference\":\"...\"}]}.";

#[derive(Debug, Deserialize)]
struct RawMatches {
    #[serde(default)]
    matches: Vec<Pair>,
}

fn listing(items: &[Item]) -> String {
    items
        .iter()
        .map(|i| format!("- {}: {}", i.name, truncate_chars(&i.description, 240)))
        .collect::<Vec<_>>()
        .join("\n")
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

#[derive(Debug, Deserialize)]
struct RawRatings {
    #[serde(default)]
    ratings: Vec<RawRating>,
}

#[derive(Debug, Deserialize)]
struct RawRating {
    id: usize,
    business: bool,
}

/// The whole judgement of the run in `repo_root`: proposals for the unmatched domains and features
/// (against `matches`, the hand-written pairs) and the rating of the narratives. `None` when the run
/// has no clustering.
///
/// # Errors
///
/// Same as [`super::matching::compare`], or if an LLM call fails.
pub async fn judge(
    llm: &dyn LlmProvider,
    repo_root: &Path,
    reference: &Reference,
    matches: &Matches,
    matches_path: &Path,
) -> Result<Option<Judgement>, PipelineError> {
    let Some(comparison) = compare(repo_root, reference, matches, matches_path)? else {
        return Ok(None);
    };
    let map = DomainMap::load(repo_root).unwrap_or_default();
    let features = load_features(repo_root).unwrap_or_default();
    let generated_domains: Vec<Item> = map
        .domains
        .iter()
        .map(|d| Item {
            name: d.name.clone(),
            description: d.description.clone(),
        })
        .collect();
    let generated_features: Vec<Item> = features
        .iter()
        .map(|f| Item {
            name: f.name.clone(),
            description: f.description.clone(),
        })
        .collect();
    let reference_domains: Vec<Item> = reference
        .domains
        .iter()
        .map(|d| Item {
            name: d.name.clone(),
            description: d.description.clone(),
        })
        .collect();
    let reference_features: Vec<Item> = reference
        .domains
        .iter()
        .flat_map(|d| &d.features)
        .map(|f| Item {
            name: f.name.clone(),
            description: f.description.clone(),
        })
        .collect();

    let domains = propose_matches(
        llm,
        "domains",
        &only(&generated_domains, &comparison.domains.unmatched_generated),
        &only(&reference_domains, &comparison.domains.unmatched_reference),
    )
    .await?;
    let features = propose_matches(
        llm,
        "features",
        &only(
            &generated_features,
            &comparison.features.unmatched_generated,
        ),
        &only(
            &reference_features,
            &comparison.features.unmatched_reference,
        ),
    )
    .await?;
    let narratives: Vec<String> = load_use_cases(repo_root)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|u| u.narrative)
        .collect();
    Ok(Some(Judgement {
        domains,
        features,
        narratives: rate_narratives(llm, &narratives).await?,
    }))
}

/// The `items` named in `names`.
fn only(items: &[Item], names: &[String]) -> Vec<Item> {
    items
        .iter()
        .filter(|i| names.contains(&i.name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeLlm;

    fn item(name: &str, description: &str) -> Item {
        Item {
            name: name.to_string(),
            description: description.to_string(),
        }
    }

    fn pair(generated: &str, reference: &str) -> Pair {
        Pair {
            generated: generated.to_string(),
            reference: reference.to_string(),
        }
    }

    #[tokio::test]
    async fn keeps_only_the_proposals_that_name_offered_items() {
        let llm = FakeLlm::answering(
            r#"{"matches":[
                {"generated":"Billing","reference":"Payments"},
                {"generated":"Invented","reference":"Payments"},
                {"generated":"Shipping","reference":"Invented too"},
                {"generated":"billing","reference":"Shipping"}
            ]}"#,
        );

        let pairs = propose_matches(
            &llm,
            "domains",
            &[item("Billing", "Invoices"), item("Shipping", "Parcels")],
            &[item("Payments", "Getting paid")],
        )
        .await
        .unwrap();

        assert_eq!(pairs, [pair("Billing", "Payments")]);
        let prompt = &llm.prompts()[0];
        assert!(
            prompt.contains("Invoices") && prompt.contains("Getting paid"),
            "{prompt}"
        );
    }

    #[tokio::test]
    async fn no_call_when_there_is_nothing_to_pair() {
        let llm = FakeLlm::answering("{}");
        assert_eq!(
            propose_matches(&llm, "features", &[], &[item("A", "a")])
                .await
                .unwrap(),
            []
        );
        assert_eq!(
            propose_matches(&llm, "features", &[item("A", "a")], &[])
                .await
                .unwrap(),
            []
        );
        assert_eq!(llm.calls(), 0);
    }

    #[tokio::test]
    async fn an_unparseable_answer_proposes_nothing() {
        let llm = FakeLlm::answering("no idea");
        let pairs = propose_matches(&llm, "domains", &[item("A", "a")], &[item("B", "b")])
            .await
            .unwrap();
        assert_eq!(pairs, []);
    }

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

    #[test]
    fn manual_pairs_keep_priority_over_proposals() {
        let matches = Matches {
            domains: vec![pair("Billing", "Payments")],
            features: vec![],
        };
        let judgement = Judgement {
            domains: vec![pair("Billing", "Shipping"), pair("Parcels", "Shipping")],
            features: vec![pair("Checkout", "Pay")],
            narratives: NarrativeRating::default(),
        };

        let merged = matches.with_proposals(&judgement);

        assert_eq!(
            merged.domains,
            [
                pair("Billing", "Payments"),
                pair("Billing", "Shipping"),
                pair("Parcels", "Shipping")
            ]
        );
        assert_eq!(merged.features, [pair("Checkout", "Pay")]);
    }

    #[tokio::test]
    async fn judges_a_run_end_to_end() {
        use crate::artifact::{save_yaml, Artifact};
        use crate::domains::{DomainCluster, DomainMap};
        use crate::features::save_features;
        use crate::use_cases::save_use_cases;
        use retrodoc_core::model::{Feature, UseCase};

        let reference: Reference = serde_yaml::from_str(
            "repository: r\ncommit: c\npurpose: p\nactors: []\ndomains:\n  - name: Payments\n    description: Getting paid\n    features:\n      - {name: Pay, description: Pay a bill}\n",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let map = DomainMap {
            domains: vec![DomainCluster {
                slug: "billing".to_string(),
                name: "Billing".to_string(),
                description: "Invoices".to_string(),
                paths: vec![],
                sub_domains: vec![],
            }],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        save_features(
            root,
            &[Feature {
                slug: "checkout".to_string(),
                domain_slug: "billing".to_string(),
                sub_domain_slug: None,
                name: "Checkout".to_string(),
                description: "Paying".to_string(),
                source_paths: vec![],
                confidence: None,
            }],
        )
        .unwrap();
        save_use_cases(
            root,
            &[UseCase {
                slug: "u".to_string(),
                feature_slug: "checkout".to_string(),
                name: "u".to_string(),
                description: String::new(),
                steps: vec![],
                entry_points: vec![],
                primary_actor: None,
                narrative: Some("A customer pays an invoice.".to_string()),
                business_language: None,
                diagram_mermaid: None,
                confidence: None,
            }],
        )
        .unwrap();
        let llm = FakeLlm::sequence(&[
            r#"{"matches":[{"generated":"Billing","reference":"Payments"}]}"#,
            r#"{"matches":[{"generated":"Checkout","reference":"Pay"}]}"#,
            r#"{"ratings":[{"id":1,"business":true}]}"#,
        ]);

        let judgement = judge(
            &llm,
            root,
            &reference,
            &Matches::default(),
            Path::new("m.yaml"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(judgement.domains, [pair("Billing", "Payments")]);
        assert_eq!(judgement.features, [pair("Checkout", "Pay")]);
        assert_eq!(
            judgement.narratives,
            NarrativeRating {
                total: 1,
                rated: 1,
                business: 1
            }
        );
        assert_eq!(llm.calls(), 3);

        let nothing = tempfile::tempdir().unwrap();
        let none = judge(
            &llm,
            nothing.path(),
            &reference,
            &Matches::default(),
            Path::new("m.yaml"),
        )
        .await
        .unwrap();
        assert_eq!(none, None, "nothing generated, nothing to judge");
    }
}
