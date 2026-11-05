//! The answers of `retrodoc ask` on a question set (spec `ask-question-set`,
//! `issues/ask_question_set.md`): each question asked to the agent on a saved run, with or without
//! the code tools, then judged by one LLM call (right, wrong or not documented; a blank answer is not
//! documented by rule) and its source hit by rule. Example `ask_replay`.

use std::fmt::Write as _;
use std::sync::Arc;

use retrodoc_agent::{ask_with, Tools};
use retrodoc_core::model::Feature;
use retrodoc_llm::{CallTotals, LlmProvider};
use retrodoc_mcp::{Docs, Source, SourceKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::matching::Matches;
use super::questions::{Question, QuestionKind, QuestionSet};
use crate::error::PipelineError;
use crate::naming::normalize;
use crate::response::complete_json;

/// Where a question stands against the saved run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    /// A documented question whose reference feature a generated feature of the run stands for.
    Covered,
    /// A documented question whose reference feature no generated feature of the run stands for.
    Uncovered,
    /// A question on what the application does not do.
    Undocumented,
}

impl Group {
    const ALL: [Group; 3] = [Group::Covered, Group::Uncovered, Group::Undocumented];

    fn label(self) -> &'static str {
        match self {
            Group::Covered => "covered",
            Group::Uncovered => "uncovered",
            Group::Undocumented => "undocumented",
        }
    }
}

/// How an answer was judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Right,
    Wrong,
    /// The answer only says the docs do not cover the question (by the judge), or is blank (by rule).
    NotDocumented,
    /// The question could not be asked (the agent's call failed: `answered` is false), or its
    /// answer could not be judged (the judge's call failed, or its reply could not be read).
    Failed,
}

impl Verdict {
    const ALL: [Verdict; 4] = [
        Verdict::Right,
        Verdict::Wrong,
        Verdict::NotDocumented,
        Verdict::Failed,
    ];

    fn label(self) -> &'static str {
        match self {
            Verdict::Right => "right",
            Verdict::Wrong => "wrong",
            Verdict::NotDocumented => "not documented",
            Verdict::Failed => "failed",
        }
    }
}

/// One answer and its judgement, as saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerRecord {
    pub id: String,
    pub group: Group,
    pub verdict: Verdict,
    /// Why the judge said right or wrong; the error of a failed question.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// For a covered question: whether a source is a generated feature standing for the expected
    /// one, or a use case of one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hit: Option<bool>,
    /// Whether the agent answered; false when its call failed. A saved record without it answered.
    #[serde(default = "answered_by_default")]
    pub answered: bool,
    pub rounds: usize,
    pub text: String,
    pub sources: Vec<String>,
    pub warnings: Vec<String>,
}

fn answered_by_default() -> bool {
    true
}

/// The generated features of the run that stand for each reference feature: the hand pairs of
/// `matches.yaml`, the judge's not rejected, and the equal names.
#[derive(Debug, Clone, Default)]
pub struct RunPairs {
    /// `(generated feature id, reference feature name)`.
    pairs: Vec<(String, String)>,
}

impl RunPairs {
    /// The pairs of `matches` (with the judge's proposals already added, [`Matches::with_proposals`])
    /// whose generated name is a feature of the run, and the features of the run named like a
    /// reference feature.
    #[must_use]
    pub fn new(matches: &Matches, features: &[Feature], reference_features: &[&str]) -> Self {
        let mut pairs = Vec::new();
        for feature in features {
            let name = normalize(&feature.name);
            let id = retrodoc_mcp::corpus::feature_id(feature);
            let paired = matches
                .features
                .iter()
                .filter(|p| normalize(&p.generated) == name)
                .map(|p| p.reference.clone());
            let equal = reference_features
                .iter()
                .filter(|r| normalize(r) == name)
                .map(ToString::to_string);
            for reference in paired.chain(equal) {
                let pair = (id.clone(), reference);
                if !pairs.contains(&pair) {
                    pairs.push(pair);
                }
            }
        }
        Self { pairs }
    }

    fn stand_for<'a>(&'a self, reference: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        let wanted = normalize(reference);
        self.pairs
            .iter()
            .filter(move |(_, r)| normalize(r) == wanted)
            .map(|(id, _)| id.as_str())
    }

    /// The group of `question` against the run.
    #[must_use]
    pub fn group(&self, question: &Question) -> Group {
        match question.kind() {
            Some(QuestionKind::Documented(feature)) if self.stand_for(feature).next().is_some() => {
                Group::Covered
            }
            Some(QuestionKind::Documented(_)) => Group::Uncovered,
            _ => Group::Undocumented,
        }
    }

    /// Whether one of `sources` is a generated feature standing for `reference`, or a use case of
    /// one (its id starts with the feature's id and a `/`).
    #[must_use]
    pub fn source_hit(&self, sources: &[Source], reference: &str) -> bool {
        self.stand_for(reference).any(|id| {
            sources.iter().any(|s| match s.kind {
                SourceKind::Feature => s.id == id,
                SourceKind::UseCase => {
                    s.id.strip_prefix(id)
                        .is_some_and(|rest| rest.starts_with('/'))
                }
                _ => false,
            })
        })
    }
}

const JUDGE_SYSTEM_PROMPT: &str = "You judge the answer of an assistant to a question about an \
application, against the expected answer written by someone who knows the application. The \
verdict is \"right\" when the answer agrees with the expected answer on what the question asks: \
it may say more, or say it in other words, or say that a detail is not documented, but it must \
not contradict the expected answer nor claim the application does something the expected answer \
says it does not. When the expected answer says the application does not do something, an answer \
saying the application does not do it, or that nothing shows it does, is right. The verdict is \
\"not documented\" when the answer only says the documentation does not cover the question, \
without answering it. Any other answer is \"wrong\". Reply in JSON: {\"verdict\": \"right\", \
\"wrong\" or \"not documented\", \"reason\": \"<one sentence>\"}.";

#[derive(Debug, Deserialize, JsonSchema)]
struct RawVerdict {
    verdict: RawKind,
    reason: String,
}

/// The verdicts the judge may give; any other reply can't be read.
#[derive(Debug, Deserialize, JsonSchema)]
enum RawKind {
    #[serde(rename = "right", alias = "Right")]
    Right,
    #[serde(rename = "wrong", alias = "Wrong")]
    Wrong,
    #[serde(
        rename = "not documented",
        alias = "Not documented",
        alias = "not_documented",
        alias = "not-documented"
    )]
    NotDocumented,
}

/// Judges the text of an answer to `question`: a blank text is not documented by rule; any other
/// takes the verdict of one call to `judge` (right, wrong or not documented), with its reason. A
/// reply of the judge that can't be read leaves the answer failed, with that reason.
///
/// # Errors
///
/// Returns the error of the call to the judge.
pub async fn judge_answer(
    judge: &dyn LlmProvider,
    question: &Question,
    text: &str,
) -> Result<(Verdict, Option<String>), PipelineError> {
    if text.trim().is_empty() {
        return Ok((Verdict::NotDocumented, None));
    }
    let user = format!(
        "Question: {}\n\nExpected answer: {}\n\nAnswer of the assistant:\n{}",
        question.question, question.expected, text
    );
    let raw = complete_json::<RawVerdict>(
        judge,
        JUDGE_SYSTEM_PROMPT,
        &user,
        "benchmark judge (answer)",
    )
    .await?;
    let Some(raw) = raw else {
        return Ok((
            Verdict::Failed,
            Some("judge: its reply could not be read".into()),
        ));
    };
    let verdict = match raw.verdict {
        RawKind::Right => Verdict::Right,
        RawKind::Wrong => Verdict::Wrong,
        RawKind::NotDocumented => Verdict::NotDocumented,
    };
    Ok((verdict, Some(raw.reason)))
}

/// Judges again the saved `records` of `set` with `judge`, keeping the answers: a question the
/// agent did not answer stays failed, an answer whose judgement failed is judged again, and one
/// whose new judgement fails keeps its former verdict. A record no longer in `set` is left as it was.
pub async fn rejudge(judge: &dyn LlmProvider, set: &QuestionSet, records: &mut [AnswerRecord]) {
    for record in records.iter_mut() {
        if !record.answered {
            continue;
        }
        let Some(question) = set.questions.iter().find(|q| q.id == record.id) else {
            continue;
        };
        match judge_answer(judge, question, &record.text).await {
            Ok((Verdict::Failed, _)) | Err(_) if record.verdict != Verdict::Failed => {}
            Ok((verdict, reason)) => {
                record.verdict = verdict;
                record.reason = reason;
            }
            Err(error) => record.reason = Some(format!("judge: {error}")),
        }
    }
}

/// Asks each question of `set` to `agent` over `docs` with the tools `offered`, then judges each
/// answer with `judge`. A question whose call fails, to the agent or to the judge, is recorded as
/// failed and the run goes on.
pub async fn ask_questions(
    agent: &dyn LlmProvider,
    judge: &dyn LlmProvider,
    docs: &Arc<Docs>,
    set: &QuestionSet,
    pairs: &RunPairs,
    offered: Tools,
    max_rounds: usize,
) -> Vec<AnswerRecord> {
    let mut records = Vec::new();
    for question in &set.questions {
        let group = pairs.group(question);
        let mut record = AnswerRecord {
            id: question.id.clone(),
            group,
            verdict: Verdict::Failed,
            reason: None,
            source_hit: None,
            answered: false,
            rounds: 0,
            text: String::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
        };
        match ask_with(
            agent,
            Arc::clone(docs),
            &question.question,
            max_rounds,
            offered,
        )
        .await
        {
            Ok(answer) => {
                record.answered = true;
                record.rounds = answer.rounds;
                record.sources = answer.sources.iter().map(ToString::to_string).collect();
                record.warnings = answer.warnings.iter().map(ToString::to_string).collect();
                if let (Group::Covered, Some(QuestionKind::Documented(feature))) =
                    (group, question.kind())
                {
                    record.source_hit = Some(pairs.source_hit(&answer.sources, feature));
                }
                match judge_answer(judge, question, &answer.text).await {
                    Ok((verdict, reason)) => {
                        record.verdict = verdict;
                        record.reason = reason;
                    }
                    Err(error) => record.reason = Some(format!("judge: {error}")),
                }
                record.text = answer.text;
            }
            Err(error) => record.reason = Some(error.to_string()),
        }
        tracing::info!(
            "{} ({}): {}",
            record.id,
            group.label(),
            record.verdict.label()
        );
        records.push(record);
    }
    records
}

/// The answers of one variant and the calls of its agent.
#[derive(Debug, Clone)]
pub struct Variant<'a> {
    pub name: &'a str,
    pub records: &'a [AnswerRecord],
    pub agent: CallTotals,
}

/// The table of the variants: per group, the count of each verdict and the source hits; then the
/// mean rounds and the calls and tokens of the agent of each variant, and the judge's.
#[must_use]
pub fn ask_table(variants: &[Variant<'_>], judge: &CallTotals) -> String {
    let mut out = String::from("| variant | group | questions |");
    for verdict in Verdict::ALL {
        let _ = write!(out, " {} |", verdict.label());
    }
    out.push_str(" source hits |\n|---|---|---|");
    out.push_str(&"---|".repeat(Verdict::ALL.len() + 1));
    out.push('\n');
    for variant in variants {
        for group in Group::ALL {
            let of_group: Vec<_> = variant
                .records
                .iter()
                .filter(|r| r.group == group)
                .collect();
            let _ = write!(
                out,
                "| {} | {} | {} |",
                variant.name,
                group.label(),
                of_group.len()
            );
            for verdict in Verdict::ALL {
                let count = of_group.iter().filter(|r| r.verdict == verdict).count();
                let _ = write!(out, " {count} |");
            }
            let hits = if group == Group::Covered {
                let hits = of_group
                    .iter()
                    .filter(|r| r.source_hit == Some(true))
                    .count();
                format!("{hits}/{}", of_group.len())
            } else {
                "-".into()
            };
            let _ = writeln!(out, " {hits} |");
        }
    }
    out.push_str("\n| calls of | mean rounds | calls | prompt tokens (cached) | completion tokens |\n|---|---|---|---|---|\n");
    for variant in variants {
        let answered = variant.records.iter().filter(|r| r.answered).count();
        #[allow(clippy::cast_precision_loss)]
        let rounds = if answered == 0 {
            0.0
        } else {
            variant
                .records
                .iter()
                .filter(|r| r.answered)
                .map(|r| r.rounds)
                .sum::<usize>() as f64
                / answered as f64
        };
        let _ = writeln!(
            out,
            "| {} | {rounds:.1} | {} |",
            variant.name,
            totals(&variant.agent)
        );
    }
    let _ = writeln!(out, "| judge | - | {} |", totals(judge));
    out
}

fn totals(totals: &CallTotals) -> String {
    format!(
        "{} | {} ({}) | {}",
        totals.calls, totals.prompt_tokens, totals.cached_tokens, totals.completion_tokens
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::matching::Pair;
    use crate::testing::{FakeLlm, FakeTurn};
    use retrodoc_core::model::{ConfidenceScore, Domain};
    use retrodoc_llm::{UsageProvider, UsageTracker};

    fn feature(domain: &str, slug: &str, name: &str) -> Feature {
        Feature {
            back_office: false,
            technical: false,
            slug: slug.into(),
            domain_slug: domain.into(),
            name: name.into(),
            description: format!("{name}."),
            source_paths: vec!["bookmarks/views.py".into()],
            entry_points: Vec::new(),
            confidence: None,
        }
    }

    fn features() -> Vec<Feature> {
        vec![
            feature("bookmarks", "bookmark-sharing", "Bookmark sharing"),
            feature("bookmarks", "tags", "Tags"),
        ]
    }

    fn docs() -> Arc<Docs> {
        let domain = Domain {
            slug: "bookmarks".into(),
            name: "Bookmarks".into(),
            description: "Saved links".into(),
            confidence: Some(ConfidenceScore::new(0.9, None)),
        };
        Arc::new(Docs::new(
            vec![domain],
            features(),
            Vec::new(),
            &retrodoc_pipeline::Glossary::default(),
            &[],
        ))
    }

    fn pairs() -> RunPairs {
        let matches = Matches {
            features: vec![Pair {
                generated: "Bookmark sharing".into(),
                reference: "Share bookmarks".into(),
            }],
            ..Matches::default()
        };
        RunPairs::new(
            &matches,
            &features(),
            &["Share bookmarks", "Tag bookmarks", "Tags", "Import"],
        )
    }

    fn question(id: &str, feature: Option<&str>) -> Question {
        Question {
            id: id.into(),
            question: format!("Question {id}?"),
            expected: format!("Expected {id}."),
            feature: feature.map(Into::into),
            absent_terms: feature.is_none().then(|| vec!["totp".into()]),
        }
    }

    fn source(kind: SourceKind, id: &str) -> Source {
        Source {
            kind,
            id: id.into(),
        }
    }

    // spec: AQS-5 (internal: the run is a saved one, which only an LLM run produces through the binary)
    #[test]
    fn a_documented_question_is_covered_when_a_feature_of_the_run_stands_for_its_feature() {
        let pairs = pairs();
        assert_eq!(
            pairs.group(&question("share", Some("Share bookmarks"))),
            Group::Covered
        );
        assert_eq!(pairs.group(&question("tags", Some("Tags"))), Group::Covered);
        assert_eq!(
            pairs.group(&question("import", Some("Import"))),
            Group::Uncovered
        );
        assert_eq!(pairs.group(&question("otp", None)), Group::Undocumented);
    }

    // spec: AQS-8 (internal: the run is a saved one, which only an LLM run produces through the binary)
    #[test]
    fn a_source_hits_when_it_is_the_feature_or_one_of_its_use_cases() {
        let pairs = pairs();
        let hit = |sources: &[Source]| pairs.source_hit(sources, "Share bookmarks");
        assert!(hit(&[source(
            SourceKind::Feature,
            "bookmarks/bookmark-sharing"
        )]));
        assert!(hit(&[source(
            SourceKind::UseCase,
            "bookmarks/bookmark-sharing/share-with-guests"
        )]));
        assert!(!hit(&[source(
            SourceKind::UseCase,
            "bookmarks/bookmark-sharing-old/x"
        )]));
        assert!(!hit(&[
            source(SourceKind::Feature, "bookmarks/tags"),
            source(SourceKind::File, "bookmarks/bookmark-sharing"),
        ]));
    }

    // spec: AQS-6 (internal: the judge is the fake provider, which the binary cannot take)
    #[tokio::test]
    async fn a_blank_answer_is_not_documented_by_rule() {
        let judge = FakeLlm::answering(r#"{"verdict":"right","reason":"r"}"#);
        let q = question("pdf", None);
        assert_eq!(
            judge_answer(&judge, &q, "  \n").await.unwrap(),
            (Verdict::NotDocumented, None)
        );
        assert_eq!(judge.calls(), 0);
    }

    // spec: AQS-7 (internal: the judge is the fake provider, which the binary cannot take)
    #[tokio::test]
    async fn any_other_answer_takes_the_verdict_of_one_call_to_the_judge() {
        let judge = FakeLlm::sequence(&[
            r#"{"verdict":"right","reason":"Same as expected."}"#,
            r#"{"verdict":"wrong","reason":"Claims a PDF export."}"#,
            r#"{"verdict":"not documented","reason":"Says the docs are silent."}"#,
        ]);
        let q = question("pdf", None);
        let answers = [
            "No, it can't; how it prints is not documented.",
            "Yes, from the menu.",
            "Not documented: I searched for PDF export.",
        ];
        let mut verdicts = Vec::new();
        for text in answers {
            verdicts.push(judge_answer(&judge, &q, text).await.unwrap().0);
        }
        assert_eq!(
            verdicts,
            [Verdict::Right, Verdict::Wrong, Verdict::NotDocumented]
        );
        assert_eq!(judge.calls(), 3);
        let prompt = &judge.prompts()[0];
        assert!(
            prompt.contains("Question pdf?")
                && prompt.contains("Expected pdf.")
                && prompt.contains("No, it can't;"),
            "{prompt}"
        );
        let system = &judge.system_prompts()[0];
        assert!(
            system.contains("does not do it") && system.contains("without answering it"),
            "{system}"
        );
    }

    // spec: AQS-7 (internal: the judge is the fake provider, which the binary cannot take)
    #[tokio::test]
    async fn the_verdict_is_one_the_judge_may_give_or_the_answer_is_failed() {
        let judge = FakeLlm::sequence(&[
            r#"{"verdict":"Right","reason":"Agrees."}"#,
            r#"{"verdict":"correct","reason":"Agrees."}"#,
            r#"{"verdict":"correct","reason":"Agrees."}"#,
        ]);
        let q = question("pdf", None);
        assert_eq!(
            judge_answer(&judge, &q, "No.").await.unwrap().0,
            Verdict::Right
        );
        assert_eq!(
            judge_answer(&judge, &q, "No.").await.unwrap(),
            (
                Verdict::Failed,
                Some("judge: its reply could not be read".into())
            )
        );
    }

    fn saved(id: &str, verdict: Verdict, answered: bool) -> AnswerRecord {
        AnswerRecord {
            id: id.into(),
            group: Group::Undocumented,
            verdict,
            reason: None,
            source_hit: None,
            answered,
            rounds: 1,
            text: if answered {
                "No.".into()
            } else {
                String::new()
            },
            sources: Vec::new(),
            warnings: Vec::new(),
        }
    }

    // spec: AQS-7 (internal: the judge is the fake provider, which the binary cannot take)
    #[tokio::test]
    async fn saved_answers_are_judged_again_and_unanswered_ones_kept() {
        let judge = FakeLlm::answering(r#"{"verdict":"right","reason":"Agrees."}"#);
        let set = QuestionSet {
            questions: vec![
                question("a", None),
                question("b", None),
                question("c", None),
            ],
        };
        let mut records = vec![
            saved("a", Verdict::NotDocumented, true),
            saved("b", Verdict::Failed, false),
            saved("c", Verdict::Failed, true),
        ];
        rejudge(&judge, &set, &mut records).await;
        let verdicts: Vec<_> = records.iter().map(|r| r.verdict).collect();
        assert_eq!(verdicts, [Verdict::Right, Verdict::Failed, Verdict::Right]);
        assert_eq!(records[0].reason.as_deref(), Some("Agrees."));
        assert_eq!(judge.calls(), 2);
    }

    // spec: AQS-7 (internal: the judge is the fake provider, which the binary cannot take)
    #[tokio::test]
    async fn a_failed_rejudgement_keeps_the_former_verdict() {
        let judge = FakeLlm::failing_after(0, |_| String::new());
        let set = QuestionSet {
            questions: vec![question("a", None)],
        };
        let mut records = vec![saved("a", Verdict::Right, true)];
        rejudge(&judge, &set, &mut records).await;
        assert_eq!(records[0].verdict, Verdict::Right);
    }

    // spec: AQS-9, AQS-11 (internal: the agent and the judge are fake providers, which the binary cannot take)
    #[tokio::test]
    async fn a_failed_question_is_recorded_and_the_run_goes_on() {
        let agent = FakeLlm::scripted(vec![
            FakeTurn::Calls {
                text: String::new(),
                calls: vec![retrodoc_llm::ToolCall {
                    id: "c1".into(),
                    name: "get_feature".into(),
                    arguments: r#"{"id":"bookmarks/bookmark-sharing"}"#.into(),
                }],
            },
            FakeTurn::Text("Share it: `bookmarks/bookmark-sharing`.".into()),
            FakeTurn::Fail,
            FakeTurn::Text("Not documented.".into()),
        ]);
        let judge = FakeLlm::answering(r#"{"verdict":"right","reason":"Agrees."}"#);
        let set = QuestionSet {
            questions: vec![
                question("share", Some("Share bookmarks")),
                question("import", Some("Import")),
                question("otp", None),
            ],
        };
        let records = ask_questions(&agent, &judge, &docs(), &set, &pairs(), Tools::All, 8).await;
        let summary: Vec<_> = records
            .iter()
            .map(|r| (r.id.as_str(), r.group, r.verdict, r.source_hit))
            .collect();
        assert_eq!(
            summary,
            [
                ("share", Group::Covered, Verdict::Right, Some(true)),
                ("import", Group::Uncovered, Verdict::Failed, None),
                ("otp", Group::Undocumented, Verdict::Right, None),
            ]
        );
        assert!(records[1].reason.is_some());
        assert!(!records[1].answered && records[0].answered && records[2].answered);
        let share = &records[0];
        assert_eq!(share.rounds, 1);
        assert_eq!(share.reason.as_deref(), Some("Agrees."));
        assert_eq!(share.sources, ["feature `bookmarks/bookmark-sharing`"]);
        assert_eq!(share.text, "Share it: `bookmarks/bookmark-sharing`.");
    }

    // spec: AQS-10 (internal: the figures come from fake providers, which the binary cannot take)
    #[tokio::test]
    async fn the_table_counts_the_verdicts_per_group_and_the_calls_apart() {
        let record = |id: &str, group, verdict, source_hit, rounds| AnswerRecord {
            id: id.into(),
            group,
            verdict,
            reason: None,
            source_hit,
            answered: verdict != Verdict::Failed,
            rounds,
            text: String::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
        };
        let records = [
            record("a", Group::Covered, Verdict::Right, Some(true), 2),
            record("b", Group::Covered, Verdict::Wrong, Some(false), 4),
            record("c", Group::Undocumented, Verdict::NotDocumented, None, 3),
            record("d", Group::Uncovered, Verdict::Failed, None, 0),
        ];
        let tracker = UsageTracker::new();
        let agent = UsageProvider::new(FakeLlm::answering("x"), tracker.clone());
        agent
            .complete(retrodoc_llm::CompletionRequest::default())
            .await
            .unwrap();
        let table = ask_table(
            &[Variant {
                name: "all tools",
                records: &records,
                agent: tracker.report().total(),
            }],
            &CallTotals::default(),
        );
        assert!(
            table.contains("| all tools | covered | 2 | 1 | 1 | 0 | 0 | 1/2 |"),
            "{table}"
        );
        assert!(
            table.contains("| all tools | uncovered | 1 | 0 | 0 | 0 | 1 | - |"),
            "{table}"
        );
        assert!(
            table.contains("| all tools | undocumented | 1 | 0 | 0 | 1 | 0 | - |"),
            "{table}"
        );
        assert!(table.contains("| all tools | 3.0 | 1 |"), "{table}");
        assert!(table.contains("| judge | - | 0 |"), "{table}");
    }
}
