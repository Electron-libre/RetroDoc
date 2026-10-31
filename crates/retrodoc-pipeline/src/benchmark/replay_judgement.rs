//! Replay of the judgement of the rules on the rules a run stated (`issues/rules_plumbing_and_screen_noise.md`).
//!
//! The statements stay fixed, so what a variant of the judgement drops is not mixed with the variance of
//! the statement call. The saved `rules-stated.yaml` holds the rules the judgement of the run kept: a
//! replay tells what a variant drops on top of it. Each feature is judged as the pass judges it
//! ([`crate::rules`]), with the system prompt given and, as an option, the rules of the other features on
//! the same files after its own (not asked by the pass: measured, it did not help); hand labels then tell
//! the good rules dropped from the noise dropped.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use futures_util::stream::{self, StreamExt, TryStreamExt};
use retrodoc_core::model::{BusinessRule, Feature};
use retrodoc_llm::LlmProvider;

use crate::error::PipelineError;
use crate::rules::{cited_code, judge_prompt, judge_rules_with, kept, one_line};

/// Rules of other features shown at most with the rules of a feature: the first ones in the order of
/// the run, not the closest.
const MAX_NEIGHBOURS: usize = 20;

/// The class of a rule the judgement keeps.
pub const KEPT: &str = "business";

/// What one replay of the judgement gave.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JudgementFigures {
    /// The class of each rule judged, by slug: [`KEPT`] unless the answer classes it as noise.
    pub classes: BTreeMap<String, String>,
    /// Features whose answer could not be read: their rules count as kept.
    pub unread: usize,
    /// Rules of no feature of the run, not judged.
    pub orphans: usize,
}

impl JudgementFigures {
    /// The rules dropped, by class of noise.
    #[must_use]
    pub fn dropped_by_class(&self) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for class in self.classes.values().filter(|c| *c != KEPT) {
            *counts.entry(class.as_str()).or_default() += 1;
        }
        counts
    }
}

/// Judges the `rules` of each of `features` under `system`, `concurrency` features at once. With
/// `neighbours`, each feature is shown, for context, the rules of the other features that cite one of
/// the files its own rules cite.
///
/// # Errors
///
/// Returns an error if an LLM call fails.
pub async fn replay_judgement(
    llm: &dyn LlmProvider,
    system: &str,
    features: &[Feature],
    rules: &[BusinessRule],
    neighbours: bool,
    concurrency: usize,
) -> Result<JudgementFigures, PipelineError> {
    let groups: Vec<(&Feature, Vec<BusinessRule>)> = features
        .iter()
        .map(|f| (f, kept(rules, &f.slug)))
        .filter(|(_, own)| !own.is_empty())
        .collect();
    let judged: usize = groups.iter().map(|(_, own)| own.len()).sum();
    let answers: Vec<_> = stream::iter(groups.iter().map(|(feature, own)| async move {
        let mut user = judge_prompt(feature, own);
        if neighbours {
            push_neighbours(&mut user, &neighbours_of(feature, own, rules));
        }
        judge_rules_with(llm, system, &user, &feature.slug, own)
            .await
            .map(|verdict| (own, verdict))
    }))
    .buffered(concurrency.max(1))
    .try_collect()
    .await?;
    let mut figures = JudgementFigures {
        orphans: rules.len() - judged,
        ..JudgementFigures::default()
    };
    for (own, verdict) in answers {
        let Some((kept, dropped)) = verdict else {
            figures.unread += 1;
            for rule in own {
                figures.classes.insert(rule.slug.clone(), KEPT.to_string());
            }
            continue;
        };
        for rule in kept {
            figures.classes.insert(rule.slug, KEPT.to_string());
        }
        for (rule, class) in dropped {
            figures.classes.insert(rule.slug, class);
        }
    }
    Ok(figures)
}

/// The rules of `all` stated by other features than `feature` that cite a file one of `own` cites.
fn neighbours_of<'a>(
    feature: &Feature,
    own: &[BusinessRule],
    all: &'a [BusinessRule],
) -> Vec<&'a BusinessRule> {
    let files: BTreeSet<&str> = own.iter().flat_map(cited_files).collect();
    all.iter()
        .filter(|r| !r.belongs_to_feature(&feature.slug))
        .filter(|r| cited_files(r).any(|path| files.contains(path)))
        .collect()
}

fn cited_files(rule: &BusinessRule) -> impl Iterator<Item = &str> {
    cited_code(rule).into_iter().map(|(path, _, _)| path)
}

/// Adds `neighbours` to the judgement prompt `user`, for context only.
fn push_neighbours(user: &mut String, neighbours: &[&BusinessRule]) {
    if neighbours.is_empty() {
        return;
    }
    user.push_str(
        "\nRules of other features stated on the same files, for context only (do not classify them):\n",
    );
    for rule in neighbours.iter().take(MAX_NEIGHBOURS) {
        let _ = writeln!(user, "- {}", one_line(&rule.statement));
    }
}

/// The label of a good rule; any other label is noise (plumbing, screen, test, duplicate).
pub const GOOD: &str = "good";

/// The judgement of a replay against hand labels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LabelScore {
    /// Labelled rules judged, good ones, and good ones dropped.
    pub labelled: usize,
    pub good: usize,
    pub good_dropped: usize,
    /// Labelled noise and the part of it dropped.
    pub noise: usize,
    pub noise_dropped: usize,
    /// Labels of a rule the replay did not judge.
    pub missing: usize,
}

/// Scores `figures` on `labels` (rule slug to label).
#[must_use]
pub fn score_labels(figures: &JudgementFigures, labels: &BTreeMap<String, String>) -> LabelScore {
    let mut score = LabelScore::default();
    for (slug, label) in labels {
        let Some(class) = figures.classes.get(slug) else {
            score.missing += 1;
            continue;
        };
        score.labelled += 1;
        let dropped = class != KEPT;
        if label == GOOD {
            score.good += 1;
            score.good_dropped += usize::from(dropped);
        } else {
            score.noise += 1;
            score.noise_dropped += usize::from(dropped);
        }
    }
    score
}

/// Reads labels written one per line as `<rule slug>\t<label>`; blank lines and lines starting with
/// `#` are skipped.
#[must_use]
pub fn parse_labels(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let (slug, label) = line.split_once('\t')?;
            Some((slug.trim().to_string(), label.trim().to_lowercase()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeLlm;
    use retrodoc_core::model::{RuleEvidence, RuleKind, SourceRef};

    fn feature(slug: &str) -> Feature {
        Feature {
            back_office: false,
            slug: slug.into(),
            domain_slug: "d".into(),
            name: slug.into(),
            description: "d".into(),
            source_paths: Vec::new(),
            entry_points: Vec::new(),
            confidence: None,
        }
    }

    fn rule(slug: &str) -> BusinessRule {
        BusinessRule {
            slug: slug.into(),
            statement: format!("statement of {slug}"),
            kind: RuleKind::Validation,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: Vec::new(),
            confidence: None,
        }
    }

    #[tokio::test]
    async fn each_rule_gets_the_class_of_the_answer_of_its_feature() {
        let llm = FakeLlm::replying(|_, request| {
            let user = &request.messages.last().unwrap().content;
            Ok(if user.contains("Feature: orders —") {
                r#"{"rules":[{"n":1,"class":"business"},{"n":2,"class":"storage"},{"n":3,"class":"odd"}]}"#
            } else {
                "not json"
            }
            .to_string())
        });
        let features = [feature("orders"), feature("orders-admin"), feature("empty")];
        let rules = [
            rule("orders-1"),
            rule("orders-2"),
            rule("orders-3"),
            rule("orders-admin-1"),
            rule("gone-1"),
        ];
        let figures = replay_judgement(&llm, "system", &features, &rules, false, 2)
            .await
            .unwrap();
        let class = |slug: &str| figures.classes[slug].as_str();
        assert_eq!(class("orders-1"), KEPT);
        assert_eq!(class("orders-2"), "storage");
        assert_eq!(class("orders-3"), KEPT, "an unknown class keeps the rule");
        assert_eq!(
            class("orders-admin-1"),
            KEPT,
            "an unread answer keeps the rules"
        );
        assert_eq!(figures.unread, 1);
        assert_eq!(figures.orphans, 1);
        assert_eq!(figures.dropped_by_class(), BTreeMap::from([("storage", 1)]));
        assert!(llm.system_prompts().iter().all(|system| system == "system"));
        // The unreadable answer is asked twice; the feature with no rule never.
        assert_eq!(llm.calls(), 3);
    }

    fn citing(slug: &str, path: &str) -> BusinessRule {
        BusinessRule {
            evidence: vec![RuleEvidence::Code {
                source: SourceRef {
                    path: path.into(),
                    start_line: Some(1),
                    end_line: Some(2),
                },
            }],
            ..rule(slug)
        }
    }

    #[tokio::test]
    async fn with_neighbours_a_feature_sees_the_rules_of_others_on_its_files() {
        let llm = FakeLlm::answering(r#"{"rules":[]}"#);
        let features = [feature("orders"), feature("riders"), feature("menus")];
        let rules = [
            citing("orders-1", "router.rb"),
            citing("riders-1", "router.rb"),
            citing("menus-1", "menu.rb"),
        ];
        replay_judgement(&llm, "system", &features, &rules, true, 1)
            .await
            .unwrap();
        let orders = &llm.prompts()[0];
        assert!(orders.contains("for context only"), "{orders}");
        assert!(orders.contains("statement of riders-1"), "{orders}");
        assert!(!orders.contains("statement of menus-1"), "{orders}");
        assert!(
            !llm.prompts()[2].contains("for context only"),
            "no neighbour, no list"
        );
    }

    #[test]
    fn labels_tell_the_good_rules_dropped_from_the_noise_dropped() {
        let figures = JudgementFigures {
            classes: BTreeMap::from([
                ("a-1".to_string(), KEPT.to_string()),
                ("a-2".to_string(), "storage".to_string()),
                ("a-3".to_string(), "screen".to_string()),
                ("a-4".to_string(), KEPT.to_string()),
            ]),
            ..JudgementFigures::default()
        };
        let labels = parse_labels(
            "# sample\na-1\tgood\na-2\tGood\na-3\tplumbing\n\na-4\tscreen\nb-1\tgood\n",
        );
        assert_eq!(
            score_labels(&figures, &labels),
            LabelScore {
                labelled: 4,
                good: 2,
                good_dropped: 1,
                noise: 2,
                noise_dropped: 1,
                missing: 1,
            }
        );
    }
}
