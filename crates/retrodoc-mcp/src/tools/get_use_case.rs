//! The `get_use_case` tool.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, UseCase};

use super::answer::{confidence_line, location, SET_APART_NOTE};
use super::Docs;
use crate::corpus::use_case_id;
use crate::reviews::Section;

impl Docs {
    /// A use case, by its id (`billing/pay-invoice/pay-by-card`), the last two segments or its
    /// slug alone when no other use case has it, with its business narrative, steps and cited
    /// code. Every segment given must match.
    #[must_use]
    pub fn get_use_case(&self, id: &str) -> String {
        let mut segments = id.rsplitn(3, '/');
        let slug = segments.next().unwrap_or_default();
        let feature_slug = segments.next();
        let domain_slug = segments.next();
        let found: Vec<&UseCase> = self
            .generated
            .use_cases()
            .iter()
            .filter(|u| {
                u.slug == slug
                    && feature_slug.is_none_or(|f| u.feature_slug == f)
                    && domain_slug.is_none_or(|d| {
                        self.generated
                            .features()
                            .iter()
                            .any(|f| f.slug == u.feature_slug && f.domain_slug == d)
                    })
            })
            .collect();
        let use_case = match found.as_slice() {
            [] => return self.unknown("use case", id, "get_feature"),
            [one] => *one,
            several => {
                let ids: Vec<String> = several
                    .iter()
                    .map(|u| format!("`{}`", use_case_id(u, self.generated.features())))
                    .collect();
                return format!(
                    "Not documented: `{id}` names several use cases: {}. Use one of these ids.\n",
                    ids.join(", ")
                );
            }
        };
        let review = self.review_note(&Section::use_case(&use_case.feature_slug, &use_case.slug));
        let mut out = format!(
            "{}# {} (`{}`)\n\n{}\n{}\n",
            review.warning,
            use_case.name,
            use_case_id(use_case, self.generated.features()),
            confidence_line(use_case.confidence.as_ref()),
            review.line
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
            .generated
            .rules()
            .iter()
            .filter(|r| r.belongs_to_feature(&use_case.feature_slug))
            .filter(|r| r.use_cases.contains(&use_case.slug))
            .collect();
        out.push_str(&self.rules_section(&rules));
        out
    }
}
