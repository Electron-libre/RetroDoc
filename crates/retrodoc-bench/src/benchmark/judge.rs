//! The LLM judge of the benchmark. It reads what the deterministic matching left out:
//!
//! - it proposes pairs between the generated domains/features, the ones already paired included (a
//!   generated item may hold several reference items), and the reference items that no generated item
//!   covers (a pair is a proposal: the user reads it, fixes it and freezes it in `matches.yaml`);
//! - a second call proposes pairs between the generated items still unpaired and the reference items the
//!   first call was not offered, so a second item of a covered reference item does not count as imprecise
//!   because `matches.yaml` holds more pairs;
//! - it rates whether each use case narrative reads as business language, in batches.
//!
//! The judge shares the biases of the model that generated the docs, so its verdicts are saved apart
//! (`judge.yaml`) and the figures built on them are reported next to the ones that are not.

use std::collections::BTreeSet;
use std::path::Path;

use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use super::matching::{compare, saved_features, scored_domains, Comparison, Matches, Pair};
use super::reference::Reference;
use crate::domains::DomainMap;
use crate::error::PipelineError;
use crate::naming::normalize;
use crate::response::complete_json;
use crate::text::truncate_chars;
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

/// The kinds whose judge call proposed no pair, twice, though there was something to pair: their
/// judged figures are left out rather than read at the strict floor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unjudged {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub domains: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub features: bool,
}

impl Unjudged {
    /// Why a kind is unjudged, for the tables and the summary that show it.
    pub const REASON: &'static str = "the judge proposed no pair, twice";

    /// Empties the recall and precision of the unjudged kinds of `judged`, the comparison made with the
    /// judge's proposals: read at the strict floor, they would move a series because the judge answered
    /// nothing.
    pub fn leave_out(self, judged: &mut Comparison) {
        for (left_out, score) in [
            (self.domains, &mut judged.domains),
            (self.features, &mut judged.features),
        ] {
            if left_out {
                score.recall = None;
                score.precision = None;
            }
        }
    }

    /// Every kind was judged.
    #[must_use]
    pub fn is_none(&self) -> bool {
        !self.domains && !self.features
    }
}

/// The kinds whose second call ([`second_calls`]) ran; a `judge.yaml` saved before it existed has none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecondCall {
    #[serde(default)]
    pub domains: bool,
    #[serde(default)]
    pub features: bool,
}

/// What the judge said about one run: written to `judge.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Judgement {
    /// The pairs of both calls.
    pub domains: Vec<Pair>,
    pub features: Vec<Pair>,
    pub narratives: NarrativeRating,
    /// Absent from the file when every kind was judged, as in the files saved before it existed.
    #[serde(default, skip_serializing_if = "Unjudged::is_none")]
    pub unjudged: Unjudged,
    #[serde(default)]
    pub second_call: SecondCall,
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

    /// The judged kinds whose second call ([`second_calls`]) never ran, as in a `judge.yaml` saved
    /// before it existed: their judged precision is biased low by the pairs of `matches.yaml`.
    #[must_use]
    pub fn without_second_call(&self) -> Vec<&'static str> {
        [
            ("domains", self.second_call.domains, self.unjudged.domains),
            (
                "features",
                self.second_call.features,
                self.unjudged.features,
            ),
        ]
        .into_iter()
        .filter(|(_, ran, unjudged)| !ran && !unjudged)
        .map(|(kind, _, _)| kind)
        .collect()
    }

    /// Reads the `judge.yaml` saved by an earlier [`judge`] of the run in `repo_root`, to re-score the
    /// run with no LLM call.
    ///
    /// # Errors
    ///
    /// Returns an error naming the file if it is missing, can't be read or parsed.
    pub fn load(repo_root: &Path) -> Result<Self, PipelineError> {
        let path = Self::path(repo_root);
        let raw = std::fs::read_to_string(&path).map_err(|source| PipelineError::ArtifactIo {
            path: path.clone(),
            source,
        })?;
        serde_yaml::from_str(&raw).map_err(|error| PipelineError::InvalidReference {
            path,
            reason: error.to_string(),
        })
    }
}

/// How many of the judge's proposals the hand audit has not seen (neither a hand pair nor a rejected
/// one), by kind; `None` for a kind not judged, or in a report saved before the count existed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unaudited {
    pub domains: Option<usize>,
    pub features: Option<usize>,
}

impl Unaudited {
    /// No count for either kind.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.domains.is_none() && self.features.is_none()
    }
}

impl Matches {
    /// The hand-written pairs followed by the judge's proposals but the rejected ones; every pair
    /// counts, so a generated name may stand for a reference item of each.
    #[must_use]
    pub fn with_proposals(&self, judgement: &Judgement) -> Matches {
        let kept = |hand: &[Pair], proposals: &[Pair], rejected: &[Pair]| -> Vec<Pair> {
            let proposals = proposals
                .iter()
                .filter(|p| !rejected.iter().any(|r| r.same_as(p)));
            hand.iter().chain(proposals).cloned().collect()
        };
        Matches {
            domains: kept(&self.domains, &judgement.domains, &self.rejected.domains),
            features: kept(&self.features, &judgement.features, &self.rejected.features),
            rejected: self.rejected.clone(),
        }
    }

    /// The judge's proposals that are neither a hand pair nor a rejected one, by judged kind.
    #[must_use]
    pub fn unaudited(&self, judgement: &Judgement) -> Unaudited {
        let count = |unjudged: bool, proposals: &[Pair], hand: &[Pair], rejected: &[Pair]| {
            (!unjudged).then(|| {
                proposals
                    .iter()
                    .filter(|p| !hand.iter().chain(rejected).any(|h| h.same_as(p)))
                    .count()
            })
        };
        Unaudited {
            domains: count(
                judgement.unjudged.domains,
                &judgement.domains,
                &self.domains,
                &self.rejected.domains,
            ),
            features: count(
                judgement.unjudged.features,
                &judgement.features,
                &self.features,
                &self.rejected.features,
            ),
        }
    }
}

/// Asks the judge to pair `generated` items with `reference` ones that stand for the same business
/// capability; a generated item that holds several reference items may get a pair for each. A name
/// copied with its description stands for the offered name; pairs naming an item that wasn't
/// offered are dropped, and so are repeated pairs. An answer with no pair left is asked once more,
/// and the second one is kept. No call when either side is empty.
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
    for _ in 0..2 {
        let pairs = ask_matches(llm, what, &user, generated, reference).await?;
        if !pairs.is_empty() {
            return Ok(pairs);
        }
    }
    tracing::warn!(what, "benchmark judge proposed no pair, twice");
    Ok(Vec::new())
}

/// One answer of the judge, its pairs read against the offered items.
async fn ask_matches(
    llm: &dyn LlmProvider,
    what: &str,
    user: &str,
    generated: &[Item],
    reference: &[Item],
) -> Result<Vec<Pair>, PipelineError> {
    let Some(answer) =
        complete_json::<RawMatches>(llm, MATCH_SYSTEM_PROMPT, user, "benchmark judge (matches)")
            .await?
    else {
        return Ok(Vec::new());
    };
    let mut pairs: Vec<Pair> = Vec::new();
    let mut dropped = 0;
    for raw in answer.matches {
        let offered = offered_name(generated, &raw.generated)
            .zip(offered_name(reference, &raw.reference))
            .map(|(generated, reference)| Pair {
                generated: generated.to_string(),
                reference: reference.to_string(),
            });
        if let Some(pair) = offered {
            if !pairs.contains(&pair) {
                pairs.push(pair);
            }
        } else {
            dropped += 1;
            tracing::debug!(
                generated = %raw.generated,
                reference = %raw.reference,
                "benchmark judge: proposal dropped"
            );
        }
    }
    if dropped > 0 {
        tracing::warn!(
            what,
            dropped,
            kept = pairs.len(),
            "benchmark judge: proposals dropped, naming an item not offered"
        );
    }
    Ok(pairs)
}

/// The offered item that `name` stands for: same normalized name, or the name followed by `:` and
/// its description (or the start of it), as the judge sometimes copies the listing line whole.
fn offered_name<'a>(items: &'a [Item], name: &str) -> Option<&'a str> {
    let same = |a: &str, b: &str| normalize(a) == normalize(b);
    let starts = |text: &str, description: &str| {
        let (text, description) = (normalize(text), normalize(description));
        !text.is_empty() && (description.starts_with(&text) || text.starts_with(&description))
    };
    if let Some(item) = items.iter().find(|i| same(&i.name, name)) {
        return Some(&item.name);
    }
    // The longest prefix first: an offered name may hold a `:` itself.
    name.rmatch_indices(':')
        .find_map(|(at, _)| {
            let (head, rest) = (&name[..at], &name[at + 1..]);
            items
                .iter()
                .find(|i| same(&i.name, head) && starts(rest, &i.description))
        })
        .map(|i| i.name.as_str())
}

const MATCH_SYSTEM_PROMPT: &str = "You are checking generated software documentation against a \
hand-written reference. You get the generated items and the reference items, each with a \
description. Pair a generated item with a reference item only when both describe the same \
business capability, even if their names differ. Leave out what has no counterpart: an \
unpaired item is a normal answer. A generated item that clearly does what several reference items \
describe is paired with each of them; several generated items may share one reference item, and a \
generated item dedicated to a reference item is paired with it even when a broader one holds it too. Copy the names exactly as written. Reply \
with ONLY a single JSON object, no prose and no Markdown code fence, matching this shape: \
{\"matches\":[{\"generated\":\"...\",\"reference\":\"...\"}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
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

/// The generated and reference items of a run, as shown to the judge.
struct RunItems {
    generated_domains: Vec<Item>,
    generated_features: Vec<Item>,
    reference_domains: Vec<Item>,
    reference_features: Vec<Item>,
}

impl RunItems {
    fn load(repo_root: &Path, reference: &Reference) -> Self {
        let map = DomainMap::load(repo_root).unwrap_or_default();
        let saved = saved_features(repo_root);
        let item = |name: &str, description: &str| Item {
            name: name.to_string(),
            description: description.to_string(),
        };
        RunItems {
            generated_domains: scored_domains(&map, saved.as_deref())
                .into_iter()
                .map(|d| item(&d.name, &d.description))
                .collect(),
            generated_features: saved
                .unwrap_or_default()
                .iter()
                .map(|f| item(&f.name, &f.description))
                .collect(),
            reference_domains: reference
                .domains
                .iter()
                .map(|d| item(&d.name, &d.description))
                .collect(),
            reference_features: reference
                .domains
                .iter()
                .flat_map(|d| &d.features)
                .map(|f| item(&f.name, &f.description))
                .collect(),
        }
    }
}

/// The whole judgement of the run in `repo_root`: proposals for the reference domains and features
/// left uncovered by `matches`, the hand-written pairs, the [`second_calls`] and the rating of the
/// narratives. `None` when the run has no clustering.
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
    let items = RunItems::load(repo_root, reference);
    let offered_domains = only(
        &items.reference_domains,
        &comparison.domains.unmatched_reference,
    );
    let domains =
        propose_matches(llm, "domains", &items.generated_domains, &offered_domains).await?;
    let offered_features = only(
        &items.reference_features,
        &comparison.features.unmatched_reference,
    );
    let features = propose_matches(
        llm,
        "features",
        &items.generated_features,
        &offered_features,
    )
    .await?;
    // `propose_matches` only comes back empty from a call after asking twice.
    let unjudged = |pairs: &[Pair], generated: &[Item], offered: &[Item]| {
        pairs.is_empty() && !generated.is_empty() && !offered.is_empty()
    };
    let unjudged = Unjudged {
        domains: unjudged(&domains, &items.generated_domains, &offered_domains),
        features: unjudged(&features, &items.generated_features, &offered_features),
    };
    let narratives: Vec<String> = load_use_cases(repo_root)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|u| u.narrative)
        .collect();
    let mut judgement = Judgement {
        domains,
        features,
        narratives: rate_narratives(llm, &narratives).await?,
        unjudged,
        second_call: SecondCall::default(),
    };
    second_calls(
        llm,
        repo_root,
        reference,
        matches,
        matches_path,
        &mut judgement,
    )
    .await?;
    Ok(Some(judgement))
}

/// The second call of each kind not made yet in `judgement`: the generated items that neither a pair
/// of `matches`, an equal name nor a proposal of the first call covers, against the reference items
/// the first call was not offered (the ones `matches` covers). Its pairs are added to `judgement`, the
/// kind is marked as having had it; an unjudged kind gets no call and no mark. A call that proposes no
/// pair is a normal answer: it doesn't mark the kind unjudged.
///
/// # Errors
///
/// Same as [`super::matching::compare`], or if an LLM call fails.
pub async fn second_calls(
    llm: &dyn LlmProvider,
    repo_root: &Path,
    reference: &Reference,
    matches: &Matches,
    matches_path: &Path,
    judgement: &mut Judgement,
) -> Result<(), PipelineError> {
    let before = compare(repo_root, reference, matches, matches_path)?;
    let after = compare(
        repo_root,
        reference,
        &matches.with_proposals(judgement),
        matches_path,
    )?;
    let (Some(before), Some(after)) = (before, after) else {
        return Ok(());
    };
    let items = RunItems::load(repo_root, reference);
    if !judgement.second_call.domains && !judgement.unjudged.domains {
        let generated = only(&items.generated_domains, &after.domains.unmatched_generated);
        let offered = except(
            &items.reference_domains,
            &before.domains.unmatched_reference,
        );
        let pairs = propose_matches(llm, "domains", &generated, &offered).await?;
        add_new(&mut judgement.domains, pairs);
        judgement.second_call.domains = true;
    }
    if !judgement.second_call.features && !judgement.unjudged.features {
        let generated = only(
            &items.generated_features,
            &after.features.unmatched_generated,
        );
        let offered = except(
            &items.reference_features,
            &before.features.unmatched_reference,
        );
        let pairs = propose_matches(llm, "features", &generated, &offered).await?;
        add_new(&mut judgement.features, pairs);
        judgement.second_call.features = true;
    }
    Ok(())
}

/// Adds to `pairs` the `new` ones it doesn't hold yet.
fn add_new(pairs: &mut Vec<Pair>, new: Vec<Pair>) {
    for pair in new {
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
}

/// The `items` named in `names`.
fn only(items: &[Item], names: &[String]) -> Vec<Item> {
    items
        .iter()
        .filter(|i| names.contains(&i.name))
        .cloned()
        .collect()
}

/// The `items` not named in `names`.
fn except(items: &[Item], names: &[String]) -> Vec<Item> {
    items
        .iter()
        .filter(|i| !names.contains(&i.name))
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

    // spec: BSC-5 (internal: the scoring needs an LLM judge and a generated run through the binary)
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

    // spec: BSC-5 (internal: the scoring needs an LLM judge and a generated run through the binary)
    #[tokio::test]
    async fn keeps_several_reference_items_for_one_generated_name() {
        let llm = FakeLlm::answering(
            r#"{"matches":[
                {"generated":"Billing","reference":"Payments"},
                {"generated":"Billing","reference":"Refunds"},
                {"generated":"billing","reference":"payments"}
            ]}"#,
        );

        let pairs = propose_matches(
            &llm,
            "features",
            &[item("Billing", "Invoices and refunds")],
            &[
                item("Payments", "Getting paid"),
                item("Refunds", "Paying back"),
            ],
        )
        .await
        .unwrap();

        assert_eq!(
            pairs,
            [pair("Billing", "Payments"), pair("Billing", "Refunds")],
            "the repeated pair is dropped"
        );
    }

    // spec: BSC-14 (internal: the judge's answer comes from a fake LLM)
    #[tokio::test]
    async fn reads_a_name_copied_with_its_description_as_the_name() {
        let llm = FakeLlm::answering(
            r#"{"matches":[
                {"generated":"Order clearing: Removes a customer's orders from the router.","reference":"Cancel a customer's order: Forget the order of a customer"},
                {"generated":"order clearing: Removes a customer's orders","reference":"Cancel a customer's order"},
                {"generated":"Order clearingness: no","reference":"Cancel a customer's order"}
            ]}"#,
        );

        let pairs = propose_matches(
            &llm,
            "features",
            &[item(
                "Order clearing",
                "Removes a customer's orders from the router.",
            )],
            &[item(
                "Cancel a customer's order",
                "Forget the order of a customer",
            )],
        )
        .await
        .unwrap();

        assert_eq!(
            pairs,
            [pair("Order clearing", "Cancel a customer's order")],
            "the description is cut off, the repeated pair and the longer name are dropped"
        );

        let llm = FakeLlm::answering(
            r#"{"matches":[{"generated":"Auth: SSO login: Signs in","reference":"Login"}]}"#,
        );
        let pairs = propose_matches(
            &llm,
            "features",
            &[item("Auth", "a"), item("Auth: SSO login", "Signs in")],
            &[item("Login", "c")],
        )
        .await
        .unwrap();
        assert_eq!(
            pairs,
            [pair("Auth: SSO login", "Login")],
            "the longest offered name wins"
        );

        let llm = FakeLlm::answering(
            r#"{"matches":[{"generated":"Refund flow","reference":"Payments: refunds"}]}"#,
        );
        let pairs = propose_matches(
            &llm,
            "features",
            &[item("Refund flow", "Pays back")],
            &[item("Payments", "Getting paid")],
        )
        .await
        .unwrap();
        assert_eq!(
            pairs,
            [],
            "a name not offered is not cut at its colon when the rest is not the description"
        );
    }

    // spec: BSC-10 (internal: the judge's answers come from a fake LLM)
    #[tokio::test]
    async fn asks_once_more_when_the_answer_proposes_no_pair() {
        let offered = || {
            (
                [item("Billing", "Invoices")],
                [item("Payments", "Getting paid")],
            )
        };
        for first in [
            r#"{"matches":[]}"#,
            r#"{"matches":[{"generated":"Invented","reference":"Payments"}]}"#,
        ] {
            let llm = FakeLlm::sequence(&[
                first,
                r#"{"matches":[{"generated":"Billing","reference":"Payments"}]}"#,
            ]);
            let (generated, reference) = offered();

            let pairs = propose_matches(&llm, "features", &generated, &reference)
                .await
                .unwrap();

            assert_eq!(pairs, [pair("Billing", "Payments")], "after {first}");
            assert_eq!(llm.calls(), 2, "after {first}");
        }

        let llm = FakeLlm::sequence(&["no idea", "no idea", r#"{"matches":[]}"#]);
        let (generated, reference) = offered();
        let pairs = propose_matches(&llm, "features", &generated, &reference)
            .await
            .unwrap();
        assert_eq!(pairs, []);
        assert_eq!(
            llm.calls(),
            3,
            "an unparseable answer is asked again, then the question once more"
        );
    }

    // spec: BSC-11 (internal: the judge's answers come from a fake LLM)
    #[tokio::test]
    async fn warns_of_the_dropped_proposals_and_of_a_call_with_no_pair() {
        let (logs, _guard) = crate::testing::capture_logs();
        let llm = FakeLlm::answering(
            r#"{"matches":[{"generated":"Invented","reference":"Payments"},{"generated":"Billing","reference":"Gone"}]}"#,
        );

        let pairs = propose_matches(
            &llm,
            "features",
            &[item("Billing", "Invoices")],
            &[item("Payments", "Getting paid")],
        )
        .await
        .unwrap();

        assert_eq!(pairs, []);
        let text = logs.text();
        assert!(
            text.contains("WARN") && text.contains("dropped=2"),
            "{text}"
        );

        let (logs, _guard) = crate::testing::capture_logs();
        let llm = FakeLlm::answering(
            r#"{"matches":[{"generated":"Billing","reference":"Payments"},{"generated":"Billing","reference":"Payments"}]}"#,
        );
        propose_matches(
            &llm,
            "features",
            &[item("Billing", "Invoices")],
            &[item("Payments", "Getting paid")],
        )
        .await
        .unwrap();
        assert!(
            !logs.text().contains("WARN"),
            "a repeated pair loses nothing: {}",
            logs.text()
        );
        assert!(
            text.contains("benchmark judge proposed no pair, twice")
                && text.contains("what=\"features\""),
            "{text}"
        );
    }

    // spec: BSC-20 (internal: the judge's answers come from a fake LLM)
    #[tokio::test]
    async fn a_domain_with_no_feature_is_not_offered_to_the_judge() {
        use crate::artifact::{save_yaml, Artifact};
        use crate::domains::{DomainCluster, DomainMap};
        use crate::features::save_features;
        use retrodoc_core::model::Feature;

        let reference: Reference = serde_yaml::from_str(
            "repository: r\ncommit: c\npurpose: p\nactors: []\ndomains:\n  - name: Payments\n    description: Getting paid\n    features:\n      - {name: Pay, description: Pay a bill}\n",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let domain = |slug: &str, name: &str| DomainCluster {
            slug: slug.to_string(),
            name: name.to_string(),
            ..DomainCluster::default()
        };
        let map = DomainMap {
            domains: vec![domain("billing", "Billing"), domain("feeds", "Feeds")],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        save_features(
            root,
            &[Feature {
                back_office: false,
                technical: false,
                slug: "checkout".to_string(),
                domain_slug: "billing".to_string(),
                name: "Checkout".to_string(),
                description: "Paying".to_string(),
                source_paths: vec![],
                entry_points: Vec::new(),
                confidence: None,
            }],
        )
        .unwrap();
        let llm =
            FakeLlm::answering(r#"{"matches":[{"generated":"Billing","reference":"Payments"}]}"#);

        judge(
            &llm,
            root,
            &reference,
            &Matches::default(),
            Path::new("m.yaml"),
        )
        .await
        .unwrap()
        .unwrap();

        let prompt = &llm.prompts()[0];
        assert!(prompt.contains("- Billing"), "{prompt}");
        assert!(!prompt.contains("Feeds"), "{prompt}");
    }

    // spec: BSC-4 (internal: the scoring needs an LLM judge and a generated run through the binary)
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

    // spec: BSC-18 (internal: the count is read from pairs built in memory)
    #[test]
    fn counts_the_proposals_neither_paired_nor_rejected_by_hand() {
        let matches = Matches {
            features: vec![pair("A", "One"), pair("b", "two")],
            rejected: crate::benchmark::Rejected {
                features: vec![pair("C", "Three")],
                ..crate::benchmark::Rejected::default()
            },
            ..Matches::default()
        };
        let judgement = Judgement {
            features: vec![
                pair("A", "One"),
                pair("B", "Two"),
                pair("C", "Three"),
                pair("D", "Four"),
                pair("E", "Five"),
            ],
            ..Judgement::default()
        };

        let unaudited = matches.unaudited(&judgement);

        assert_eq!(unaudited.features, Some(2));
        assert_eq!(unaudited.domains, Some(0));
        let unjudged = Judgement {
            unjudged: Unjudged {
                domains: false,
                features: true,
            },
            ..judgement
        };
        assert_eq!(matches.unaudited(&unjudged).features, None);
    }

    #[test]
    fn merges_the_manual_pairs_and_the_proposals() {
        let matches = Matches {
            domains: vec![pair("Billing", "Payments")],
            ..Matches::default()
        };
        let judgement = Judgement {
            domains: vec![pair("Billing", "Shipping"), pair("Parcels", "Shipping")],
            features: vec![pair("Checkout", "Pay")],
            ..Judgement::default()
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
                ..DomainCluster::default()
            }],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        save_features(
            root,
            &[Feature {
                back_office: false,
                technical: false,
                slug: "checkout".to_string(),
                domain_slug: "billing".to_string(),
                name: "Checkout".to_string(),
                description: "Paying".to_string(),
                source_paths: vec![],
                entry_points: Vec::new(),
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
                narrative_check: None,
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

    // spec: BSC-12 (internal: the judge's answers come from a fake LLM)
    #[tokio::test]
    async fn a_kind_with_no_pair_twice_is_marked_unjudged_in_the_saved_judgement() {
        use crate::artifact::{save_yaml, Artifact};
        use crate::domains::{DomainCluster, DomainMap};
        use crate::features::save_features;
        use retrodoc_core::model::Feature;

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
                ..DomainCluster::default()
            }],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        save_features(
            root,
            &[Feature {
                back_office: false,
                technical: false,
                slug: "checkout".to_string(),
                domain_slug: "billing".to_string(),
                name: "Checkout".to_string(),
                description: "Paying".to_string(),
                source_paths: vec![],
                entry_points: Vec::new(),
                confidence: None,
            }],
        )
        .unwrap();
        let llm = FakeLlm::sequence(&[
            r#"{"matches":[{"generated":"Billing","reference":"Payments"}]}"#,
            r#"{"matches":[]}"#,
            r#"{"matches":[{"generated":"Checkout: Paying","reference":"Invented"}]}"#,
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

        assert_eq!(
            judgement.unjudged,
            Unjudged {
                domains: false,
                features: true
            }
        );
        assert_eq!(
            judgement.second_call,
            SecondCall {
                domains: true,
                features: false
            },
            "an unjudged kind gets no second call"
        );
        judgement.save(root).unwrap();
        let saved = std::fs::read_to_string(Judgement::path(root)).unwrap();
        assert!(saved.contains("unjudged:\n  features: true"), "{saved}");
        assert!(!saved.contains("unjudged:\n  domains"), "{saved}");
        assert_eq!(Judgement::load(root).unwrap(), judgement);

        let judged = Judgement {
            domains: vec![pair("Billing", "Payments")],
            ..Judgement::default()
        };
        judged.save(root).unwrap();
        let saved = std::fs::read_to_string(Judgement::path(root)).unwrap();
        assert!(
            !saved.contains("unjudged"),
            "a judged run keeps the old file: {saved}"
        );
        assert_eq!(Judgement::load(root).unwrap().unjudged, Unjudged::default());
    }

    // spec: BSC-4 (internal: the scoring needs an LLM judge and a generated run through the binary)
    #[tokio::test]
    async fn offers_every_generated_feature_against_the_uncovered_reference_ones() {
        use crate::artifact::{save_yaml, Artifact};
        use crate::domains::{DomainCluster, DomainMap};
        use crate::features::save_features;
        use retrodoc_core::model::Feature;

        let reference = |features: &str| -> Reference {
            serde_yaml::from_str(&format!(
                "repository: r\ncommit: c\npurpose: p\nactors: []\ndomains:\n  - name: Billing\n    description: d\n    features:\n{features}"
            ))
            .unwrap()
        };
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let map = DomainMap {
            domains: vec![DomainCluster {
                slug: "billing".to_string(),
                name: "Billing".to_string(),
                ..DomainCluster::default()
            }],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        let feature = |name: &str, description: &str| Feature {
            back_office: false,
            technical: false,
            slug: name.to_lowercase(),
            domain_slug: "billing".to_string(),
            name: name.to_string(),
            description: description.to_string(),
            source_paths: vec![],
            entry_points: Vec::new(),
            confidence: None,
        };
        save_features(
            root,
            &[
                feature("Checkout", "Paying, and refunds too"),
                feature("Newsletter", "Mails"),
            ],
        )
        .unwrap();
        let llm = FakeLlm::sequence(&[
            r#"{"matches":[{"generated":"Checkout","reference":"Refund"}]}"#,
            r#"{"matches":[]}"#,
            r#"{"matches":[]}"#,
        ]);

        let judgement = judge(
            &llm,
            root,
            &reference(
                "      - {name: Checkout, description: Pay}\n      - {name: Refund, description: Pay back}\n",
            ),
            &Matches::default(),
            Path::new("m.yaml"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(judgement.features, [pair("Checkout", "Refund")]);
        assert_eq!(
            llm.calls(),
            3,
            "the domain is covered and paired: no call for it; the second features call asked twice"
        );
        let prompt = &llm.prompts()[0];
        let (generated, reference_part) = prompt.split_once("Reference features:").unwrap();
        assert!(
            generated.contains("- Checkout:") && generated.contains("- Newsletter:"),
            "{prompt}"
        );
        assert!(
            reference_part.contains("- Refund:") && !reference_part.contains("- Checkout:"),
            "{prompt}"
        );

        let covered = FakeLlm::answering("{}");
        judge(
            &covered,
            root,
            &reference("      - {name: Checkout, description: Pay}\n"),
            &Matches::default(),
            Path::new("m.yaml"),
        )
        .await
        .unwrap();
        assert!(
            covered
                .prompts()
                .iter()
                .all(|p| !p.contains("- Checkout: Paying")),
            "every reference item is covered: no first call, only the second one for Newsletter"
        );
    }

    /// A run with a covered reference domain and feature, and a generated domain and feature left
    /// unpaired: the `matches.yaml` and the reference to judge it with.
    fn run_with_second_items(root: &Path) -> (Reference, Matches) {
        use crate::artifact::{save_yaml, Artifact};
        use crate::domains::{DomainCluster, DomainMap};
        use crate::features::save_features;
        use retrodoc_core::model::Feature;

        let reference: Reference = serde_yaml::from_str(
            "repository: r\ncommit: c\npurpose: p\nactors: []\ndomains:\n  - name: Administration\n    description: Run the site\n    features:\n      - {name: Manage users, description: Add and remove users}\n",
        )
        .unwrap();
        let domain = |slug: &str, name: &str| DomainCluster {
            slug: slug.to_string(),
            name: name.to_string(),
            ..DomainCluster::default()
        };
        let map = DomainMap {
            domains: vec![
                domain("settings", "Settings"),
                domain("users", "User administration"),
            ],
        };
        save_yaml(&Artifact::Domains.path(root), &map).unwrap();
        let feature = |domain: &str, name: &str| Feature {
            back_office: false,
            technical: false,
            slug: name.to_lowercase().replace(' ', "-"),
            domain_slug: domain.to_string(),
            name: name.to_string(),
            description: String::new(),
            source_paths: vec![],
            entry_points: Vec::new(),
            confidence: None,
        };
        save_features(
            root,
            &[
                feature("settings", "Manage users"),
                feature("users", "User list"),
            ],
        )
        .unwrap();
        let matches = Matches {
            domains: vec![pair("Settings", "Administration")],
            ..Matches::default()
        };
        (reference, matches)
    }

    // spec: BSC-21 (internal: the judge's answers come from a fake LLM)
    #[tokio::test]
    async fn a_second_call_pairs_the_unpaired_items_with_the_covered_reference_ones() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let (reference, matches) = run_with_second_items(root);
        let llm = FakeLlm::sequence(&[
            r#"{"matches":[{"generated":"User administration","reference":"Administration"}]}"#,
            r#"{"matches":[{"generated":"User list","reference":"Manage users"}]}"#,
        ]);

        let judgement = judge(&llm, root, &reference, &matches, Path::new("m.yaml"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            judgement.domains,
            [pair("User administration", "Administration")]
        );
        assert_eq!(judgement.features, [pair("User list", "Manage users")]);
        assert_eq!(
            judgement.second_call,
            SecondCall {
                domains: true,
                features: true
            }
        );
        assert_eq!(
            llm.calls(),
            2,
            "every reference item is covered: no first call"
        );
        let (generated, offered) = llm.prompts()[0]
            .split_once("Reference domains:")
            .map(|(g, r)| (g.to_string(), r.to_string()))
            .unwrap();
        assert!(
            generated.contains("- User administration:") && !generated.contains("- Settings:"),
            "{generated}"
        );
        assert!(offered.contains("- Administration:"), "{offered}");
        let strict = compare(root, &reference, &matches, Path::new("m.yaml"))
            .unwrap()
            .unwrap();
        let judged = compare(
            root,
            &reference,
            &matches.with_proposals(&judgement),
            Path::new("m.yaml"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(strict.domains.precision, Some(0.5));
        assert_eq!(judged.domains.precision, Some(1.0));
        assert_eq!(judged.domains.recall, strict.domains.recall);
    }

    // spec: BSC-21 (internal: the judge's answers come from a fake LLM)
    #[tokio::test]
    async fn a_second_call_with_no_pair_leaves_the_kind_judged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let (reference, matches) = run_with_second_items(root);
        let silent = FakeLlm::answering(r#"{"matches":[]}"#);
        let judgement = judge(&silent, root, &reference, &matches, Path::new("m.yaml"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(silent.calls(), 4, "each second call asked twice");
        assert_eq!(judgement.unjudged, Unjudged::default());
        assert!(judgement.domains.is_empty() && judgement.features.is_empty());
    }
}
