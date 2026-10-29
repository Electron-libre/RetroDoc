//! The section of the debt report on the project's own docs.

use std::fmt::Write as _;

use super::{one_line, DebtReport};
use crate::reconcile::{ItemVerdict, Reconciliation};

/// Undocumented items listed in the Markdown report (the rest is only counted).
const MAX_LISTED_UNDOCUMENTED: usize = 50;

/// The items a doc section contradicts, with what each side states.
fn write_contradictions(md: &mut String, items: &[&ItemVerdict]) {
    if items.is_empty() {
        return;
    }
    md.push_str(
        "\n### Contradicted\n\nThe existing docs and the code disagree on these items; the doc may \
         be out of date rather than wrong, check which one is right:\n\n",
    );
    for item in items {
        let _ = writeln!(
            md,
            "- `{}` {}: {}",
            item.label,
            item.kind.label(),
            one_line(&item.name)
        );
        for c in &item.contradictions {
            let _ = writeln!(
                md,
                "  - `{}` states: {}\n  - the generated docs state: {}",
                c.section.origin(),
                one_line(&c.doc_states),
                one_line(&c.item_states)
            );
        }
    }
}

impl DebtReport {
    /// Adds where the items stand against the project's own docs.
    pub fn with_reconciliation(&mut self, reconciliation: Reconciliation) {
        self.reconciliation = Some(reconciliation);
    }

    /// The section on the existing docs: how much of each domain they cover, then what they leave out.
    pub(super) fn write_reconciliation_section(&self, md: &mut String) {
        let Some(reconciliation) = self.reconciliation.as_ref().filter(|r| !r.items.is_empty())
        else {
            return;
        };
        md.push_str(
            "\n## Existing documentation\n\nWhat the project's own documents say about the generated \
             items (domains, features, use cases, business rules).\n\n\
             | Domain | Items | Documented | Matched, not judged | Undocumented | Contradicted |\n\
             |---|---|---|---|---|---|\n",
        );
        let mut rows: Vec<_> = reconciliation.by_domain().into_iter().collect();
        rows.sort_by_key(|(domain, count)| (std::cmp::Reverse(count.undocumented), *domain));
        for (domain, count) in rows {
            let _ = writeln!(
                md,
                "| {domain} | {} | {} | {} | {} | {} |",
                count.total(),
                count.documented,
                count.matched,
                count.undocumented,
                count.contradicted
            );
        }
        write_contradictions(md, &reconciliation.contradicted());
        let undocumented = reconciliation.undocumented();
        if undocumented.is_empty() {
            return;
        }
        let _ = write!(
            md,
            "\n### Undocumented\n\n{} item(s) are mentioned in no existing document, most entry \
             points first:\n\n",
            undocumented.len()
        );
        for item in undocumented.iter().take(MAX_LISTED_UNDOCUMENTED) {
            let plural = if item.entry_points == 1 { "" } else { "s" };
            let _ = writeln!(
                md,
                "- `{}` {}: {} ({} entry point{plural})",
                item.label,
                item.kind.label(),
                one_line(&item.name),
                item.entry_points
            );
        }
        if undocumented.len() > MAX_LISTED_UNDOCUMENTED {
            let _ = writeln!(
                md,
                "- … and {} more",
                undocumented.len() - MAX_LISTED_UNDOCUMENTED
            );
        }
    }
}
