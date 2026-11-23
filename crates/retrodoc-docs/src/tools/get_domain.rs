//! The `get_domain` tool.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Feature};

use retrodoc_core::text::one_line;

use super::answer::{confidence_line, confidence_short, rule_item, snippet};
use super::Docs;
use crate::corpus::feature_id;
use crate::reviews::{Section, SectionKind};

impl Docs {
    /// A domain (`billing`) and its features.
    #[must_use]
    pub fn get_domain(&self, id: &str) -> String {
        let Some(domain) = self.generated.domains().iter().find(|d| d.slug == id) else {
            return self.unknown("domain", id, "list_domains");
        };
        let section = Section::new(SectionKind::Domain, id);
        let review = self.review_note(&section);
        let edit = self.edit_note(&section);
        let mut out = format!(
            "{}# {} (`{id}`)\n\n{}\n\n{}\n{}\n{}",
            review.warning,
            domain.name,
            domain.description,
            confidence_line(domain.confidence.as_ref()),
            review.line,
            edit.line
        );
        let features: Vec<&Feature> = self.generated.features_of(id).collect();
        if features.is_empty() {
            out.push_str("\nNo feature documented here.\n");
            out.push_str(&edit.generated);
            return out;
        }
        let rules: Vec<&BusinessRule> = self
            .generated
            .rules()
            .iter()
            .filter(|r| features.iter().any(|f| r.belongs_to_feature(&f.slug)))
            .collect();
        out.push_str("\n## Features\n\n");
        for feature in features {
            let _ = writeln!(
                out,
                "- `{}` — {}: {} ({})",
                feature_id(feature),
                one_line(&feature.name),
                snippet(&feature.description),
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
        out.push_str(&edit.generated);
        out
    }
}
