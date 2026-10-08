//! The few signals closest to one unit of work (a domain, a feature): the
//! documentation sections, test descriptions and commit subjects that share
//! the most words with it, found with the BM25 of [`crate::bm25`]. The brief
//! frames every prompt with the same global view; this adds, per unit, the
//! local evidence. No LLM, deterministic.

use std::fmt::Write as _;

use retrodoc_ingest::signals::{Signal, SignalKind};

use super::cut_at_word;
use crate::bm25::Bm25;

/// Characters kept of one extract.
const MAX_PER_EXTRACT: usize = 400;

/// What is searched, with its title and its budget in characters for one
/// unit (about 3,000 together).
const POOLS: &[(&str, &[SignalKind], usize)] = &[
    ("Documentation", &[SignalKind::DocSection], 1_200),
    (
        "Tests",
        &[SignalKind::TestDescriptions, SignalKind::FeatureScenarios],
        900,
    ),
    ("Commits", &[SignalKind::CommitSubject], 900),
];

#[derive(Debug, Clone)]
struct Pool {
    title: &'static str,
    budget: usize,
    /// Text of each signal, in the order of the index.
    extracts: Vec<String>,
    index: Bm25,
}

/// The searchable signals. Empty (the default) retrieves nothing.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    pools: Vec<Pool>,
}

impl Evidence {
    #[must_use]
    pub fn new(signals: &[Signal]) -> Self {
        let pools = POOLS
            .iter()
            .filter_map(|(title, kinds, budget)| {
                let found: Vec<&Signal> =
                    signals.iter().filter(|s| kinds.contains(&s.kind)).collect();
                if found.is_empty() {
                    return None;
                }
                // The origin is searched (a file name says what it is about)
                // but never shown: the LLM would cite it as a source file.
                let texts: Vec<String> = found
                    .iter()
                    .map(|s| format!("{} {}", s.origin, s.text))
                    .collect();
                let extracts = found.iter().map(|s| s.text.clone()).collect();
                Some(Pool {
                    title,
                    budget: *budget,
                    index: Bm25::new(&texts),
                    extracts,
                })
            })
            .collect();
        Self { pools }
    }

    /// The prompt section with the extracts closest to `query`, best first
    /// within each kind and within its budget; empty when nothing matches.
    #[must_use]
    pub fn section(&self, query: &str) -> String {
        let mut body = String::new();
        for pool in &self.pools {
            let mut left = pool.budget;
            let mut lines = String::new();
            for (doc, _) in pool.index.search_any(query) {
                let text = &pool.extracts[doc];
                let line = format!("- {}\n", cut_at_word(text, MAX_PER_EXTRACT));
                let cost = line.chars().count();
                if cost > left {
                    continue;
                }
                left -= cost;
                lines.push_str(&line);
            }
            if !lines.is_empty() {
                let _ = write!(body, "== {} ==\n{lines}", pool.title);
            }
        }
        if body.is_empty() {
            return String::new();
        }
        format!(
            "Project evidence closest to this unit (documentation, tests and commit history; \
             they are not source files, never cite them, and they may be partly unrelated: never \
             describe what the code does not show):\n{body}\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(kind: SignalKind, origin: &str, text: &str) -> Signal {
        Signal {
            kind,
            origin: origin.to_string(),
            text: text.to_string(),
        }
    }

    fn evidence() -> Evidence {
        Evidence::new(&[
            signal(
                SignalKind::DocSection,
                "README.md#Refunds",
                "A buyer asks a refund of a paid order.",
            ),
            signal(
                SignalKind::DocSection,
                "README.md#Install",
                "Run the installer on the server.",
            ),
            signal(
                SignalKind::TestDescriptions,
                "spec/refund_spec.rb",
                "refunds a paid order",
            ),
            signal(SignalKind::CommitSubject, "commit:aaa", "feat: add refunds"),
            signal(SignalKind::Schema, "db/schema.rb", "refunds: amount"),
        ])
    }

    #[test]
    fn the_closest_extracts_of_each_kind_are_shown_without_their_origin() {
        let section = evidence().section("Refund of an order: refund_controller.rb");
        assert!(
            section.contains("== Documentation ==\n- A buyer asks a refund"),
            "{section}"
        );
        assert!(!section.contains("README.md"), "{section}");
        assert!(
            section.contains("== Tests ==\n- refunds a paid order"),
            "{section}"
        );
        assert!(
            section.contains("== Commits ==\n- feat: add refunds"),
            "{section}"
        );
        assert!(
            !section.contains("spec/") && !section.contains("commit:"),
            "{section}"
        );
        assert!(!section.contains("Install"), "{section}");
        // The schema is the brief's, not searched here.
        assert!(!section.contains("db/schema.rb"), "{section}");
    }

    #[test]
    fn nothing_close_or_nothing_collected_gives_an_empty_section() {
        assert_eq!(evidence().section("weather forecast"), "");
        assert_eq!(Evidence::default().section("refund"), "");
        assert_eq!(Evidence::new(&[]).section("refund"), "");
    }

    #[test]
    fn a_kind_stays_within_its_budget() {
        let many: Vec<Signal> = (0..100)
            .map(|i| {
                signal(
                    SignalKind::CommitSubject,
                    &format!("commit:{i}"),
                    "fix: refund rounding bug in the totals of the order",
                )
            })
            .collect();
        let section = Evidence::new(&many).section("refund");
        assert!(section.chars().count() < 1_200, "{}", section.len());
        assert!(section.contains("- fix: refund"), "{section}");
        assert!(section.lines().count() < 20, "{section}");
    }

    #[test]
    fn an_extract_too_big_for_what_is_left_does_not_hide_the_next_ones() {
        let long = "refund ".repeat(300);
        let section = Evidence::new(&[
            signal(SignalKind::DocSection, "a.md", &long),
            signal(SignalKind::DocSection, "b.md", &long),
            signal(SignalKind::DocSection, "c.md", &long),
            signal(SignalKind::DocSection, "d.md", "refund policy"),
        ])
        .section("refund");
        assert!(section.contains("- refund policy"), "{section}");
    }

    #[test]
    fn the_same_input_gives_the_same_section() {
        assert_eq!(
            evidence().section("refund order"),
            evidence().section("refund order")
        );
    }
}
