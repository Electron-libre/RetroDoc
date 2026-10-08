//! The plain-text summary `retrodoc benchmark` prints for one run.

use std::fmt::Write as _;
use std::time::Duration;

use super::judge::Judgement;
use super::matching::{Comparison, Score};
use super::metrics::RunMetrics;
use crate::progress::format_duration;

/// `Some(0.5)` → `50%`, `None` → `n/a`.
fn percent(value: Option<f32>) -> String {
    value.map_or_else(|| "n/a".to_string(), |v| format!("{:.0}%", v * 100.0))
}

fn score_line(out: &mut String, label: &str, score: &Score) {
    let _ = writeln!(
        out,
        "{label}: recall {}, precision {} ({} matched)",
        percent(score.recall),
        percent(score.precision),
        score.matched.len()
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
/// and, when the judge ran, the scores with its proposals next to its rating of the narratives.
#[must_use]
pub fn summary(
    metrics: &RunMetrics,
    comparison: &Comparison,
    judged: Option<(&Comparison, &Judgement)>,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Run: {} domain(s), {} sub-domain(s), {} feature(s), {} use case(s), {} file(s) uncategorized",
        metrics.domains,
        metrics.sub_domains,
        metrics.features,
        metrics.use_cases,
        metrics.uncategorized_files
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
    if let Some((with_judge, judgement)) = judged {
        out.push('\n');
        out.push_str("With the judge's proposals (not yet in matches.yaml):\n");
        score_line(&mut out, "Domains", &with_judge.domains);
        score_line(&mut out, "Features", &with_judge.features);
        let rating = judgement.narratives;
        let _ = writeln!(
            out,
            "Narratives in business language, by the judge: {} ({} of {} rated, {} use case(s) with a narrative)",
            percent(rating.business_share()),
            rating.business,
            rating.rated,
            rating.total
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::judge::NarrativeRating;
    use crate::benchmark::metrics::Cost;

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
            sub_domains: 1,
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

        assert!(text.contains("Run: 3 domain(s), 1 sub-domain(s), 7 feature(s), 12 use case(s), 2 file(s) uncategorized"), "{text}");
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
            },
            ..Judgement::default()
        };

        let text = summary(&metrics(None), &comparison, Some((&judged, &judgement)));

        assert!(!text.contains("Cost of"), "{text}");
        assert!(text.contains("With the judge's proposals"), "{text}");
        assert!(
            text.contains("Domains: recall 100%, precision 100%"),
            "{text}"
        );
        assert!(text.contains("Narratives in business language, by the judge: 90% (9 of 10 rated, 12 use case(s) with a narrative)"), "{text}");
    }
}
