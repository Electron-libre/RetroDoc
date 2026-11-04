//! The `get_feature` tool.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, UseCase};

use retrodoc_core::text::one_line;

use super::answer::{
    confidence_line, confidence_short, set_apart_tag, snippet, sources_section, SET_APART_NOTE,
};
use super::Docs;
use crate::corpus::feature_id;
use crate::reviews::{Section, SectionKind};

impl Docs {
    /// A feature, by its id (`billing/pay-invoice`) or its slug alone (feature slugs are unique
    /// across domains), with its use cases and the files it is grounded on.
    #[must_use]
    pub fn get_feature(&self, id: &str) -> String {
        let (domain, slug) = match id.split_once('/') {
            Some((domain, slug)) => (Some(domain), slug),
            None => (None, id),
        };
        let found = self
            .generated
            .features()
            .iter()
            .find(|f| f.slug == slug && domain.is_none_or(|d| f.domain_slug == d));
        let Some(feature) = found else {
            return self.unknown("feature", id, "get_domain");
        };
        let review = self.review_note(&Section::new(SectionKind::Feature, &feature.slug));
        let mut out = format!(
            "{}# {} (`{}`)\n\n{}\n\n{}\n{}\n",
            review.warning,
            feature.name,
            feature_id(feature),
            feature.description,
            confidence_line(feature.confidence.as_ref()),
            review.line
        );
        if feature.back_office {
            let _ = writeln!(out, "{SET_APART_NOTE}");
        }
        out.push_str(&self.stale_notice(feature.source_paths.iter().map(String::as_str)));
        out.push_str(&sources_section(
            "Grounded on",
            feature.source_paths.iter().map(String::as_str),
        ));
        let use_cases: Vec<&UseCase> = self.generated.use_cases_of(&feature.slug).collect();
        let rules: Vec<&BusinessRule> = self
            .generated
            .rules()
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
                one_line(&use_case.name),
                snippet(&use_case.description),
                confidence_short(use_case.confidence.as_ref()),
                set_apart_tag(self.use_case_set_apart(use_case)),
            );
        }
        out.push_str("\nUse `get_use_case` with an id to see the steps.\n");
        out.push_str(&self.rules_section(&rules));
        out
    }
}
