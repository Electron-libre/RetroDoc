//! The `get_feature` tool.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, UseCase};

use super::answer::{
    confidence_line, confidence_short, last_segment, one_line, set_apart_tag, sources_section,
    SET_APART_NOTE,
};
use super::Docs;
use crate::corpus::feature_id;

impl Docs {
    /// A feature, by its id (`billing/pay-invoice`) or its slug alone, with
    /// its use cases and the files it is grounded on.
    #[must_use]
    pub fn get_feature(&self, id: &str) -> String {
        let slug = last_segment(id);
        let Some(feature) = self.features.iter().find(|f| f.slug == slug) else {
            return self.unknown("feature", id, "get_domain");
        };
        let mut out = format!(
            "# {} (`{}`)\n\n{}\n\n{}\n",
            feature.name,
            feature_id(feature),
            feature.description,
            confidence_line(feature.confidence.as_ref())
        );
        if feature.back_office {
            let _ = writeln!(out, "{SET_APART_NOTE}");
        }
        out.push_str(&self.stale_notice(feature.source_paths.iter().map(String::as_str)));
        out.push_str(&sources_section(
            "Grounded on",
            feature.source_paths.iter().map(String::as_str),
        ));
        let use_cases: Vec<&UseCase> = self
            .use_cases
            .iter()
            .filter(|u| u.feature_slug == feature.slug)
            .collect();
        let rules: Vec<&BusinessRule> = self
            .rules
            .iter()
            .filter(|r| r.belongs_to_feature(&feature.slug))
            .collect();
        if use_cases.is_empty() {
            out.push_str("\nNo use case documented for this feature.\n");
            out.push_str(&self.rules_section(&rules));
            return out;
        }
        out.push_str("\n## Use cases\n\n");
        for use_case in use_cases {
            let _ = writeln!(
                out,
                "- `{}/{}` — {}: {} ({}){}",
                feature_id(feature),
                use_case.slug,
                use_case.name,
                one_line(&use_case.description),
                confidence_short(use_case.confidence.as_ref()),
                set_apart_tag(self.use_case_set_apart(use_case)),
            );
        }
        out.push_str("\nUse `get_use_case` with an id to see the steps.\n");
        out.push_str(&self.rules_section(&rules));
        out
    }
}
