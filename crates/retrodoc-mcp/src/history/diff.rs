//! The difference between two saved runs (`specs/history-page.md`): the sections one run added,
//! removed or made worse, and those it set apart under "Administration and technical" or no longer
//! does. A section is the same in both runs by its kind and id only.

use std::collections::{HashMap, HashSet};

use super::{SectionKind, SectionScores, Snapshot};

/// The smallest fall of a score shown: the p90 of the confidence of a section between two runs of
/// the same code (measured on 2026-11-04).
pub const FALL_THRESHOLD: f32 = 0.2;

/// What the `to` run changed in the sections of the `from` run.
#[derive(Debug, PartialEq)]
pub struct RunDiff<'a> {
    /// The sections of `to` that `from` lacks, in the order of `to`.
    pub added: Vec<&'a SectionScores>,
    /// The sections of `from` that `to` lacks, in the order of `from`.
    pub removed: Vec<&'a SectionScores>,
    /// The sections of `to` a score of which fell by at least [`FALL_THRESHOLD`], with their falls.
    pub degraded: Vec<(&'a SectionScores, Vec<Fall>)>,
    /// The sections of `to` set apart there and not in `from`.
    pub set_apart: Vec<&'a SectionScores>,
    /// The sections of `to` set apart in `from` and not there.
    pub no_longer_set_apart: Vec<&'a SectionScores>,
}

impl RunDiff<'_> {
    /// The two runs differ in none of their sections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.degraded.is_empty()
            && self.set_apart.is_empty()
            && self.no_longer_set_apart.is_empty()
    }
}

/// A score of a section, as the snapshot holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Score {
    Confidence,
    BusinessLanguage,
}

/// A score that fell from `from` to `to`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fall {
    pub score: Score,
    pub from: f32,
    pub to: f32,
}

impl Fall {
    #[must_use]
    pub fn by(&self) -> f32 {
        self.from - self.to
    }
}

/// The difference `to` makes to `from`.
#[must_use]
pub fn diff<'a>(from: &'a Snapshot, to: &'a Snapshot) -> RunDiff<'a> {
    let before: HashMap<_, _> = from.sections.iter().map(|s| (key(s), s)).collect();
    let after: HashSet<_> = to.sections.iter().map(key).collect();
    let mut diff = RunDiff {
        added: Vec::new(),
        removed: from
            .sections
            .iter()
            .filter(|s| !after.contains(&key(s)))
            .collect(),
        degraded: Vec::new(),
        set_apart: Vec::new(),
        no_longer_set_apart: Vec::new(),
    };
    for section in &to.sections {
        let Some(old) = before.get(&key(section)) else {
            diff.added.push(section);
            continue;
        };
        let falls: Vec<Fall> = [
            (Score::Confidence, old.confidence, section.confidence),
            (
                Score::BusinessLanguage,
                old.business_language,
                section.business_language,
            ),
        ]
        .into_iter()
        .filter_map(|(score, from, to)| {
            Some(Fall {
                score,
                from: from?,
                to: to?,
            })
        })
        // A tolerance for the rounding of `f32`: 0.9 then 0.7 is a fall of 0.2.
        .filter(|fall| fall.by() >= FALL_THRESHOLD - 1e-6)
        .collect();
        if !falls.is_empty() {
            diff.degraded.push((section, falls));
        }
        match (old.set_apart.is_empty(), section.set_apart.is_empty()) {
            (true, false) => diff.set_apart.push(section),
            (false, true) => diff.no_longer_set_apart.push(section),
            _ => {}
        }
    }
    diff
}

fn key(section: &SectionScores) -> (SectionKind, &str) {
    (section.kind, section.id.as_str())
}

#[cfg(test)]
mod tests {
    use super::super::{Figures, VERSION};
    use super::*;

    fn section(kind: SectionKind, id: &str, confidence: Option<f32>) -> SectionScores {
        SectionScores {
            kind,
            id: id.into(),
            name: id.to_uppercase(),
            confidence,
            business_language: None,
            narrative_check: None,
            set_apart: Vec::new(),
        }
    }

    fn use_case(id: &str, confidence: f32, business: Option<f32>) -> SectionScores {
        SectionScores {
            business_language: business,
            ..section(SectionKind::UseCase, id, Some(confidence))
        }
    }

    fn run(sections: Vec<SectionScores>) -> Snapshot {
        Snapshot {
            version: VERSION,
            commit: None,
            finished_at: "2026-11-04T10:00:00Z".into(),
            cost: None,
            figures: Figures {
                confidence: None,
                business_language: None,
                coverage: None,
                debt: Vec::new(),
            },
            sections,
        }
    }

    fn ids<'a>(sections: &[&'a SectionScores]) -> Vec<&'a str> {
        sections.iter().map(|s| s.id.as_str()).collect()
    }

    // spec: HPG-6, HPG-7 (internal: the difference of the model; the page shows it in deliverable 2)
    #[test]
    fn sections_are_added_and_removed_by_kind_and_id_in_the_order_of_their_run() {
        let from = run(vec![
            section(SectionKind::Domain, "billing", Some(0.8)),
            section(SectionKind::Feature, "pay", Some(0.8)),
            section(SectionKind::Feature, "billing", Some(0.8)),
            use_case("pay/refund", 0.8, None),
            use_case("pay/void", 0.8, None),
        ]);
        let to = run(vec![
            section(SectionKind::Domain, "billing", Some(0.8)),
            section(SectionKind::Domain, "auth", Some(0.8)),
            section(SectionKind::Feature, "pay", Some(0.8)),
            use_case("pay/void", 0.8, None),
            use_case("refunds/refund", 0.8, None),
            use_case("pay/cancel", 0.8, None),
        ]);

        let diff = diff(&from, &to);

        assert_eq!(ids(&diff.added), ["auth", "refunds/refund", "pay/cancel"]);
        assert_eq!(ids(&diff.removed), ["billing", "pay/refund"]);
        assert_eq!(diff.removed[0].kind, SectionKind::Feature);
    }

    // spec: HPG-8, HPG-9 (internal: the difference of the model; the page shows it in deliverable 2)
    #[test]
    fn a_section_is_degraded_when_a_score_falls_by_the_threshold_and_both_runs_have_it() {
        let from = run(vec![
            use_case("pay/small", 0.9, Some(0.9)),
            use_case("pay/large", 0.9, Some(0.9)),
            use_case("pay/edge", 0.9, None),
            use_case("pay/business", 0.9, Some(0.9)),
            use_case("pay/lost", 0.9, Some(0.9)),
            section(SectionKind::Feature, "unscored", None),
        ]);
        let to = run(vec![
            use_case("pay/small", 0.75, Some(0.9)),
            use_case("pay/large", 0.6, Some(0.95)),
            use_case("pay/edge", 0.7, Some(0.1)),
            use_case("pay/business", 1.0, Some(0.5)),
            use_case("pay/lost", 0.9, None),
            section(SectionKind::Feature, "unscored", Some(0.1)),
        ]);

        let diff = diff(&from, &to);

        let degraded: Vec<(&str, Vec<Score>)> = diff
            .degraded
            .iter()
            .map(|(s, falls)| (s.id.as_str(), falls.iter().map(|f| f.score).collect()))
            .collect();
        assert_eq!(
            degraded,
            [
                ("pay/large", vec![Score::Confidence]),
                ("pay/edge", vec![Score::Confidence]),
                ("pay/business", vec![Score::BusinessLanguage]),
            ]
        );
        let fall = diff.degraded[0].1[0];
        assert_eq!((fall.from, fall.to), (0.9, 0.6));
        assert!((fall.by() - 0.3).abs() < 1e-6);
    }

    // spec: HPG-10, HPG-11 (internal: the difference of the model; the page shows it in deliverable 2)
    #[test]
    fn a_use_case_set_apart_in_one_run_only_is_newly_or_no_longer_set_apart() {
        let apart = |id: &str, reasons: &[&str]| SectionScores {
            set_apart: reasons.iter().map(|r| (*r).to_string()).collect(),
            ..use_case(id, 0.8, None)
        };
        let from = run(vec![
            apart("pay/kept", &["confidence"]),
            apart("pay/freed", &["back_office"]),
            apart("pay/new", &[]),
            apart("pay/gone", &["confidence"]),
        ]);
        let to = run(vec![
            apart("pay/kept", &["business_language"]),
            apart("pay/freed", &[]),
            apart("pay/new", &["confidence", "business_language"]),
            apart("pay/added", &["confidence"]),
        ]);

        let diff = diff(&from, &to);

        assert_eq!(ids(&diff.set_apart), ["pay/new"]);
        assert_eq!(
            diff.set_apart[0].set_apart,
            ["confidence", "business_language"]
        );
        assert_eq!(ids(&diff.no_longer_set_apart), ["pay/freed"]);
    }

    // spec: HPG-13 (internal: the difference of the model; the page shows it in deliverable 2)
    #[test]
    fn a_run_compared_with_itself_or_a_better_one_makes_no_difference() {
        let from = run(vec![use_case("pay/refund", 0.5, Some(0.5))]);
        let better = run(vec![use_case("pay/refund", 0.9, Some(0.9))]);

        assert!(diff(&from, &from).is_empty());
        assert!(diff(&from, &better).is_empty());
        assert!(!diff(&better, &from).is_empty());
    }
}
