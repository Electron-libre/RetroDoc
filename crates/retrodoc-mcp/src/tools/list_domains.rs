//! The `list_domains` tool.

use std::fmt::Write as _;

use super::answer::{confidence_short, one_line};
use super::Docs;

impl Docs {
    /// Every domain, with the number of features.
    #[must_use]
    pub fn list_domains(&self) -> String {
        if self.domains.is_empty() {
            return "Not documented: no domain was generated.\n".into();
        }
        let mut out = String::from("# Domains\n\n");
        for domain in &self.domains {
            let count = self
                .features
                .iter()
                .filter(|f| f.domain_slug == domain.slug)
                .count();
            let _ = writeln!(
                out,
                "- `{}` — {}: {} ({}, {count} feature(s))",
                domain.slug,
                domain.name,
                one_line(&domain.description),
                confidence_short(domain.confidence.as_ref()),
            );
        }
        out.push_str("\nUse `get_domain` with an id to see its features.\n");
        out
    }
}
