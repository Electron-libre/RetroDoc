//! The `get_use_case` tool.

use std::fmt::Write as _;

use retrodoc_core::model::BusinessRule;

use super::answer::{confidence_line, location, SET_APART_NOTE};
use super::Docs;
use crate::corpus::feature_id;

impl Docs {
    /// A use case, by its id (`billing/pay-invoice/pay-by-card`) or the last
    /// two segments, with its business narrative, steps and cited code.
    #[must_use]
    pub fn get_use_case(&self, id: &str) -> String {
        let mut segments = id.rsplit('/');
        let slug = segments.next().unwrap_or_default();
        let feature_slug = segments.next();
        let found = self
            .use_cases
            .iter()
            .find(|u| u.slug == slug && feature_slug.is_none_or(|f| u.feature_slug == f));
        let Some(use_case) = found else {
            return self.unknown("use case", id, "get_feature");
        };
        let parent = self
            .features
            .iter()
            .find(|f| f.slug == use_case.feature_slug)
            .map_or_else(|| use_case.feature_slug.clone(), feature_id);
        let mut out = format!(
            "# {} (`{parent}/{}`)\n\n{}\n",
            use_case.name,
            use_case.slug,
            confidence_line(use_case.confidence.as_ref())
        );
        if self.use_case_set_apart(use_case) {
            let _ = writeln!(out, "{SET_APART_NOTE}");
        }
        let cited = use_case
            .steps
            .iter()
            .flat_map(|step| step.source_refs.iter().map(|r| r.path.as_str()));
        out.push_str(&self.stale_notice(cited));
        if let Some(narrative) = &use_case.narrative {
            let _ = writeln!(out, "\n{narrative}");
        }
        if !use_case.description.is_empty() {
            let _ = writeln!(out, "\n{}", use_case.description);
        }
        if let Some(actor) = &use_case.primary_actor {
            let _ = writeln!(out, "\nPrimary actor: {actor}");
        }
        if !use_case.entry_points.is_empty() {
            let _ = writeln!(
                out,
                "Triggered by: {}",
                use_case.entry_point_names().join(", ")
            );
        }
        out.push_str("\n## Steps\n\n");
        for step in &use_case.steps {
            let cited: Vec<String> = step.source_refs.iter().map(location).collect();
            let cited = if cited.is_empty() {
                " (no code cited)".to_string()
            } else {
                format!(" ({})", cited.join(", "))
            };
            let text = match step.detail() {
                Some(detail) => format!("{}: {detail}", step.label()),
                None => step.label().to_string(),
            };
            let _ = writeln!(out, "{}. {} — {text}{cited}", step.order, step.actor.name);
        }
        let rules: Vec<&BusinessRule> = self
            .rules
            .iter()
            .filter(|r| r.belongs_to_feature(&use_case.feature_slug))
            .filter(|r| r.use_cases.contains(&use_case.slug))
            .collect();
        out.push_str(&self.rules_section(&rules));
        out
    }
}
