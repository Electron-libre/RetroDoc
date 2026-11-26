//! The plain-text summary `retrodoc benchmark` prints for one run.

use std::fmt::Write as _;
use std::time::Duration;

use super::judge::{Judgement, Unaudited, Unjudged};
use super::matching::{Comparison, Score};
use super::metrics::RunMetrics;
use super::narratives::BatchRating;
use retrodoc_core::text::format_duration;

/// `Some(0.5)` → `50%`, `None` → `n/a`.
fn percent(value: Option<f32>) -> String {
    value.map_or_else(|| "n/a".to_string(), |v| format!("{:.0}%", v * 100.0))
}

fn score_line(out: &mut String, label: &str, score: &Score) {
    // The pairs of one generated item are next to each other: count the items, not the pairs.
    let items = score.matched.chunk_by(|a, b| a.0 == b.0).count();
    let pairs = score.matched.len();
    let matched = if pairs == items {
        format!("{items} matched")
    } else {
        format!("{items} matched, {pairs} pairs")
    };
    let _ = writeln!(
        out,
        "{label}: recall {}, precision {} ({matched})",
        percent(score.recall),
        percent(score.precision),
    );
    if !score.unmatched_generated.is_empty() {
        let _ = writeln!(
            out,
            "  generated, not in the reference: {}",
            score.unmatched_generated.join(", ")
        );
    }
    if !score.unmatched_reference.is_empty() {
        let _ = writeln!(
            out,
            "  in the reference, not generated: {}",
            score.unmatched_reference.join(", ")
        );
    }
}

/// The figures of a run, its scores against the reference (hand-written pairs and equal names only)
/// and, when the judge ran, the scores with its proposals, how many of them no hand has audited, and
/// its rating of the narratives.
#[must_use]
pub fn summary(
    metrics: &RunMetrics,
    comparison: &Comparison,
    judged: Option<(&Comparison, &Judgement, Unaudited)>,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Run: {} domain(s), {} feature(s), {} use case(s), {} file(s) uncategorized",
        metrics.domains, metrics.features, metrics.use_cases, metrics.uncategorized_files
    );
    let _ = writeln!(
        out,
        "Use cases: {} with a narrative, business-language score {}, confidence {}, {} at score 1",
        percent(metrics.narrative_share),
        percent(metrics.business_language),
        percent(metrics.confidence),
        percent(metrics.fully_business)
    );
    if let Some(cost) = metrics.cost {
        let _ = writeln!(
            out,
            "Cost of `generate`: {} call(s), {} tokens in, {} out, {}{}",
            cost.calls,
            cost.prompt_tokens,
            cost.completion_tokens,
            format_duration(Duration::from_millis(cost.wall_ms)),
            if cost.calls_without_usage > 0 {
                " (token counts are a lower bound)"
            } else {
                ""
            }
        );
    }
    out.push('\n');
    score_line(&mut out, "Domains", &comparison.domains);
    score_line(&mut out, "Features", &comparison.features);
    if let Some((with_judge, judgement, unaudited)) = judged {
        out.push('\n');
        out.push_str("With the judge's proposals (less the ones matches.yaml rejects):\n");
        for (label, score, unjudged, unaudited) in [
            (
                "Domains",
                &with_judge.domains,
                judgement.unjudged.domains,
                unaudited.domains,
            ),
            (
                "Features",
                &with_judge.features,
                judgement.unjudged.features,
                unaudited.features,
            ),
        ] {
            if unjudged {
                // As in the saved report, which holds no judged figures for the kind.
                let _ = writeln!(out, "{label}: unjudged ({})", Unjudged::REASON);
            } else {
                score_line(&mut out, label, score);
            }
            if let Some(unaudited) = unaudited {
                let _ = writeln!(
                    out,
                    "  {unaudited} of the judge's pairs not audited by hand"
                );
            }
        }
        let rating = &judgement.narratives;
        let _ = writeln!(
            out,
            "Narratives in business language, by the judge: {} ({} of {} rated, {} use case(s) with a narrative)",
            percent(rating.business_share()),
            rating.business,
            rating.rated,
            rating.total
        );
        if let Some(points) = rating.off_by(metrics.business_language) {
            let _ = writeln!(
                out,
                "  off the self-rated business-language score by {points} points: rate it again before reading it (`--judge`, which also proposes the pairs again)"
            );
            // Logged here, the one place `retrodoc benchmark` reads both figures together.
            tracing::warn!(
                points,
                "the judge's rating of the narratives is off the self-rated business-language score"
            );
        }
        for (n, batch) in rating.batches.iter().enumerate() {
            let _ = writeln!(out, "  batch {}: {}", n + 1, batch_line(batch));
        }
    }
    out
}

/// The rating of a batch of narratives, with each of its ratings when it was rated again (BNR-8).
fn batch_line(batch: &BatchRating) -> String {
    let mut line = format!(
        "{} of {} rated, {} in business language",
        batch.rated, batch.narratives, batch.business
    );
    if !batch.ratings.is_empty() {
        let each: Vec<String> = batch
            .ratings
            .iter()
            .map(|v| {
                if v.rated == 0 {
                    "no verdict".to_string()
                } else {
                    v.business.to_string()
                }
            })
            .collect();
        let _ = write!(
            line,
            " (rated {} times: {}; {} kept)",
            batch.ratings.len(),
            each.join(", "),
            batch.business
        );
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::metrics::Cost;
    use crate::benchmark::narratives::{rate_narratives, BatchRating, NarrativeRating, Verdicts};
    use crate::testing::FakeLlm;

    fn score(
        recall: Option<f32>,
        precision: Option<f32>,
        gen: &[&str],
        reference: &[&str],
    ) -> Score {
        Score {
            recall,
            precision,
            matched: vec![("a".to_string(), "b".to_string())],
            unmatched_generated: gen.iter().map(ToString::to_string).collect(),
            unmatched_reference: reference.iter().map(ToString::to_string).collect(),
        }
    }

    fn metrics(cost: Option<Cost>) -> RunMetrics {
        RunMetrics {
            domains: 3,
            features: 7,
            use_cases: 12,
            uncategorized_files: 2,
            narrative_share: Some(0.5),
            business_language: Some(0.6),
            fully_business: Some(0.5),
            confidence: None,
            cost,
        }
    }

    // spec: BSC-2 (internal: the scoring needs an LLM judge and a generated run through the binary)
    #[test]
    fn a_merged_item_counts_once_in_the_matched_count() {
        let mut merged = score(Some(1.0), Some(1.0), &[], &[]);
        merged.matched = vec![
            ("Voting".to_string(), "Upvote".to_string()),
            ("Voting".to_string(), "Flag".to_string()),
        ];
        let comparison = Comparison {
            domains: merged.clone(),
            features: merged,
        };

        let text = summary(&metrics(None), &comparison, None);

        assert!(text.contains("(1 matched, 2 pairs)"), "{text}");
    }

    #[test]
    fn summarizes_a_run_with_its_scores() {
        let comparison = Comparison {
            domains: score(Some(0.5), Some(1.0), &[], &["Shipping"]),
            features: score(None, Some(0.0), &["Newsletter"], &[]),
        };
        let cost = Cost {
            calls: 42,
            prompt_tokens: 1000,
            completion_tokens: 90,
            wall_ms: 65_000,
            calls_without_usage: 1,
        };

        let text = summary(&metrics(Some(cost)), &comparison, None);

        assert!(
            text.contains(
                "Run: 3 domain(s), 7 feature(s), 12 use case(s), 2 file(s) uncategorized"
            ),
            "{text}"
        );
        assert!(
            text.contains("50% with a narrative, business-language score 60%, confidence n/a")
                || text
                    .contains("50% with a narrative, business-language score 63%, confidence n/a"),
            "{text}"
        );
        assert!(
            text.contains(
                "42 call(s), 1000 tokens in, 90 out, 1m05s (token counts are a lower bound)"
            ),
            "{text}"
        );
        assert!(
            text.contains("Domains: recall 50%, precision 100% (1 matched)"),
            "{text}"
        );
        assert!(
            text.contains("  in the reference, not generated: Shipping"),
            "{text}"
        );
        assert!(
            text.contains("Features: recall n/a, precision 0%"),
            "{text}"
        );
        assert!(
            text.contains("  generated, not in the reference: Newsletter"),
            "{text}"
        );
        assert!(!text.contains("judge"), "{text}");
    }

    // spec: BSC-15 (internal: the binary would need a judge answering no pair twice)
    #[test]
    fn prints_an_unjudged_kind_as_unjudged() {
        let comparison = Comparison {
            domains: score(Some(0.5), Some(0.5), &[], &[]),
            features: score(Some(0.25), Some(0.25), &[], &[]),
        };
        let judged = Comparison {
            domains: score(Some(1.0), Some(1.0), &[], &[]),
            features: score(Some(0.25), Some(0.25), &[], &["Pay"]),
        };
        let judgement = Judgement {
            unjudged: Unjudged {
                features: true,
                ..Unjudged::default()
            },
            ..Judgement::default()
        };

        let text = summary(
            &metrics(None),
            &comparison,
            Some((&judged, &judgement, Unaudited::default())),
        );
        let with_judge = text.split("With the judge's proposals").nth(1).unwrap();

        assert!(
            with_judge.contains("Domains: recall 100%, precision 100%"),
            "{text}"
        );
        assert!(
            with_judge.contains("Features: unjudged (the judge proposed no pair, twice)"),
            "{text}"
        );
        assert!(!with_judge.contains("Features: recall"), "{text}");
        assert!(!with_judge.contains("Pay"), "{text}");
    }

    #[test]
    fn adds_the_judge_figures_when_it_ran() {
        let comparison = Comparison {
            domains: score(Some(0.5), Some(0.5), &[], &[]),
            features: score(Some(0.5), Some(0.5), &[], &[]),
        };
        let judged = Comparison {
            domains: score(Some(1.0), Some(1.0), &[], &[]),
            features: score(Some(0.75), Some(0.5), &[], &[]),
        };
        let judgement = Judgement {
            narratives: NarrativeRating {
                total: 12,
                rated: 10,
                business: 9,
                ..NarrativeRating::default()
            },
            ..Judgement::default()
        };

        let text = summary(
            &metrics(None),
            &comparison,
            Some((&judged, &judgement, Unaudited::default())),
        );

        assert!(!text.contains("Cost of"), "{text}");
        assert!(text.contains("With the judge's proposals"), "{text}");
        assert!(
            text.contains("Domains: recall 100%, precision 100%"),
            "{text}"
        );
        assert!(text.contains("Narratives in business language, by the judge: 90% (9 of 10 rated, 12 use case(s) with a narrative)"), "{text}");
    }

    // spec: BNR-H1, BNR-1 (internal: the binary would need an LLM judge and a generated run)
    #[tokio::test]
    async fn prints_the_rating_of_each_batch() {
        let llm = FakeLlm::sequence(&[
            r#"{"ratings":[{"id":1,"business":true},{"id":2,"business":false}]}"#,
            r#"{"ratings":[{"id":1,"business":true}]}"#,
        ]);
        let narratives: Vec<String> = (0..25).map(|i| format!("narrative {i}")).collect();
        let comparison = Comparison {
            domains: score(Some(1.0), Some(1.0), &[], &[]),
            features: score(Some(1.0), Some(1.0), &[], &[]),
        };
        let judgement = Judgement {
            narratives: rate_narratives(&llm, &narratives).await.unwrap(),
            ..Judgement::default()
        };

        let text = summary(
            &metrics(None),
            &comparison,
            Some((&comparison, &judgement, Unaudited::default())),
        );

        assert!(
            text.contains("by the judge: 67% (2 of 3 rated, 25 use case(s) with a narrative)"),
            "{text}"
        );
        assert!(
            text.contains("  batch 1: 2 of 20 rated, 1 in business language"),
            "{text}"
        );
        assert!(
            text.contains("  batch 2: 1 of 5 rated, 1 in business language"),
            "{text}"
        );
    }

    // spec: BNR-H3, BNR-8 (internal: the binary would need an LLM judge and a generated run)
    #[test]
    fn prints_each_rating_of_a_batch_rated_again() {
        let comparison = Comparison {
            domains: score(Some(1.0), Some(1.0), &[], &[]),
            features: score(Some(1.0), Some(1.0), &[], &[]),
        };
        let verdicts = |business| Verdicts {
            rated: 20,
            business,
        };
        let judgement = Judgement {
            narratives: NarrativeRating {
                total: 40,
                rated: 40,
                business: 34,
                batches: vec![
                    BatchRating {
                        narratives: 20,
                        rated: 20,
                        business: 19,
                        ..BatchRating::default()
                    },
                    BatchRating {
                        narratives: 20,
                        rated: 20,
                        business: 15,
                        ratings: vec![verdicts(0), verdicts(17), verdicts(15)],
                    },
                ],
            },
            ..Judgement::default()
        };

        let text = summary(
            &metrics(None),
            &comparison,
            Some((&comparison, &judgement, Unaudited::default())),
        );

        assert!(
            text.contains("  batch 1: 20 of 20 rated, 19 in business language\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "  batch 2: 20 of 20 rated, 15 in business language (rated 3 times: 0, 17, 15; 15 kept)"
            ),
            "{text}"
        );
    }

    // spec: BNR-2 (internal: the binary would need an LLM judge and a generated run)
    #[test]
    fn marks_and_warns_of_a_rating_off_the_self_rated_score() {
        let comparison = Comparison {
            domains: score(Some(1.0), Some(1.0), &[], &[]),
            features: score(Some(1.0), Some(1.0), &[], &[]),
        };
        let rating = |business| Judgement {
            narratives: NarrativeRating {
                total: 10,
                rated: 10,
                business,
                ..NarrativeRating::default()
            },
            ..Judgement::default()
        };
        let mut run = metrics(None);
        run.business_language = Some(0.9);
        let print = |judgement: &Judgement| {
            summary(
                &run,
                &comparison,
                Some((&comparison, judgement, Unaudited::default())),
            )
        };
        let (logs, _guard) = crate::testing::capture_logs();

        let off = print(&rating(3));
        let close = print(&rating(8));

        assert!(
            off.contains(
                "by the judge: 30% (3 of 10 rated, 10 use case(s) with a narrative)\n  \
                 off the self-rated business-language score by 60 points: rate it again before \
                 reading it"
            ),
            "{off}"
        );
        assert!(!close.contains("off the self-rated"), "{close}");
        let logs = logs.text();
        assert!(logs.contains("WARN"), "{logs}");
        assert!(logs.contains("60"), "{logs}");
        assert_eq!(logs.matches("WARN").count(), 1, "{logs}");
    }
}
