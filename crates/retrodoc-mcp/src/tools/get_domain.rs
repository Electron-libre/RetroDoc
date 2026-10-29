//! The `get_domain` tool.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Feature};

use super::answer::{confidence_line, confidence_short, one_line, rule_item};
use super::Docs;
use crate::corpus::feature_id;

impl Docs {
    /// A domain (`billing`) and its features.
    #[must_use]
    pub fn get_domain(&self, id: &str) -> String {
        let Some(domain) = self.domains.iter().find(|d| d.slug == id) else {
            return self.unknown("domain", id, "list_domains");
        };
        let mut out = format!(
            "# {} (`{id}`)\n\n{}\n\n{}\n",
            domain.name,
            domain.description,
            confidence_line(domain.confidence.as_ref())
        );
        let features: Vec<&Feature> = self
            .features
            .iter()
            .filter(|f| f.domain_slug == id)
            .collect();
        if features.is_empty() {
            out.push_str("\nNo feature documented here.\n");
            return out;
        }
        let rules: Vec<&BusinessRule> = self
            .rules
            .iter()
            .filter(|r| features.iter().any(|f| r.belongs_to_feature(&f.slug)))
            .collect();
        out.push_str("\n## Features\n\n");
        for feature in features {
            let _ = writeln!(
                out,
                "- `{}` — {}: {} ({})",
                feature_id(feature),
                feature.name,
                one_line(&feature.description),
                confidence_short(feature.confidence.as_ref()),
            );
        }
        out.push_str("\nUse `get_feature` with an id to see its use cases.\n");
        if !rules.is_empty() {
            out.push_str("\n## Business rules\n\n");
            for rule in rules {
                let _ = writeln!(out, "{}", rule_item(rule));
            }
            out.push_str("\nUse `get_feature` with a feature id to see their evidence.\n");
        }
        out
    }
}
