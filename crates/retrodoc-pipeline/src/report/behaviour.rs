//! The sections of the debt report on what the behaviours leave out (`surface` runs only).

use std::fmt::Write as _;

use super::DebtReport;
use crate::behaviour_coverage::BehaviourCoverage;

/// Technical support files listed in the Markdown report (the rest is only counted).
const MAX_LISTED_SUPPORT: usize = 50;
/// Entry points without a use case listed in the Markdown report (the rest is only counted).
const MAX_LISTED_UNCOVERED: usize = 50;

impl DebtReport {
    /// Adds what the behaviours leave out, from the saved coverage of a `surface` run.
    pub fn with_behaviour_coverage(&mut self, coverage: Option<BehaviourCoverage>) {
        if let Some(coverage) = coverage {
            self.unplaced_entry_points = coverage.unplaced_entry_points;
            self.entries_by_context = coverage.by_domain;
            self.entries_by_feature = coverage.by_feature;
            self.technical_support = coverage.technical_support;
        }
    }

    /// The sections on what the behaviours leave out (`surface` runs only).
    pub(super) fn write_behaviour_sections(&self, md: &mut String) {
        if !self.unplaced_entry_points.is_empty() {
            md.push_str(
                "\n## Entry points without a feature\n\nNo feature holds these entry points, so \
                 their behaviour is not documented:\n\n",
            );
            for id in &self.unplaced_entry_points {
                let _ = writeln!(md, "- `{id}`");
            }
        }
        if !self.entries_by_context.is_empty() {
            md.push_str(
                "\n## Entry points by context\n\n| Context | Entry points | Without a feature |\n|---|---|---|\n",
            );
            let mut rows: Vec<_> = self.entries_by_context.iter().collect();
            rows.sort_by_key(|(_, count)| std::cmp::Reverse((count.unplaced, count.entry_points)));
            for (context, count) in rows {
                let _ = writeln!(
                    md,
                    "| {context} | {} | {} |",
                    count.entry_points, count.unplaced
                );
            }
        }
        self.write_uncovered_by_use_cases(md);
        if !self.technical_support.is_empty() {
            let _ = write!(
                md,
                "\n## Technical support (reached by no behaviour)\n\n{} source file(s) hold no \
                 entry point of a feature, are not run by one and mention no entity; they are \
                 absent from the functional docs:\n\n",
                self.technical_support.len()
            );
            for path in self.technical_support.iter().take(MAX_LISTED_SUPPORT) {
                let _ = writeln!(md, "- `{}`", path.display());
            }
            if self.technical_support.len() > MAX_LISTED_SUPPORT {
                let _ = writeln!(
                    md,
                    "- … and {} more",
                    self.technical_support.len() - MAX_LISTED_SUPPORT
                );
            }
        }
    }

    /// The entry points of the features that no use case cites, per feature and overall.
    pub(super) fn write_uncovered_by_use_cases(&self, md: &mut String) {
        let total: usize = self
            .entries_by_feature
            .values()
            .map(|c| c.entry_points)
            .sum();
        let uncovered: usize = self
            .entries_by_feature
            .values()
            .map(|c| c.uncovered.len())
            .sum();
        if uncovered == 0 {
            return;
        }
        let _ = write!(
            md,
            "\n## Entry points without a use case\n\n{uncovered} of {total} entry points held by \
             a feature are cited by no use case, so what they do is not described:\n\n\
             | Feature | Entry points | Without a use case |\n|---|---|---|\n"
        );
        let mut rows: Vec<_> = self
            .entries_by_feature
            .iter()
            .filter(|(_, c)| !c.uncovered.is_empty())
            .collect();
        rows.sort_by_key(|(_, c)| std::cmp::Reverse((c.uncovered.len(), c.entry_points)));
        for (feature, count) in &rows {
            let _ = writeln!(
                md,
                "| {feature} | {} | {} |",
                count.entry_points,
                count.uncovered.len()
            );
        }
        md.push('\n');
        let ids: Vec<&String> = rows.iter().flat_map(|(_, c)| &c.uncovered).collect();
        for id in ids.iter().take(MAX_LISTED_UNCOVERED) {
            let _ = writeln!(md, "- `{id}`");
        }
        if ids.len() > MAX_LISTED_UNCOVERED {
            let _ = writeln!(md, "- … and {} more", ids.len() - MAX_LISTED_UNCOVERED);
        }
    }
}
