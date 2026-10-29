//! The section of the debt report on what the rules pass set aside.

use std::fmt::Write as _;

use retrodoc_core::config::RulesConfig;
use retrodoc_core::model::BusinessRule;

use super::{one_line, DebtReport};
use crate::rules::{AsideFeature, AsideRule, SetAside};

/// Rules set aside listed in the Markdown report (the rest is only counted).
const MAX_LISTED_RULES: usize = 50;

impl DebtReport {
    /// Adds what the rules pass set aside on a judgement of the LLM, and which of its rules `put_back`
    /// lists and are `published`. Nothing when it set nothing aside.
    pub fn with_set_aside(
        &mut self,
        set_aside: SetAside,
        put_back: &RulesConfig,
        published: &[BusinessRule],
    ) {
        self.rules_put_back = set_aside
            .rules
            .iter()
            .filter(|r| {
                put_back.keep_rules.contains(&r.slug) && published.iter().any(|p| p.slug == r.slug)
            })
            .map(|r| r.slug.clone())
            .collect();
        self.set_aside = (!set_aside.is_empty()).then_some(set_aside);
    }

    /// The section on what the rules pass set aside: the features taken for tests (they lost every
    /// rule, so a wrong answer costs the most) and the rules dropped as noise.
    pub(super) fn write_set_aside_section(&self, md: &mut String) {
        let Some(aside) = &self.set_aside else { return };
        md.push_str(
            "\n## Set aside by the rules pass (to check)\n\nThe LLM judged these; a wrong answer \
             hides a business rule. The full list is in `.retrodoc/cache/rules-set-aside.yaml`, \
             every rule as each feature stated it in `.retrodoc/cache/rules-stated.yaml`.\n",
        );
        if !aside.features.is_empty() {
            let _ = write!(
                md,
                "\n### {} feature(s) without rules\n\nTaken for features about tests, so no rule was \
                 asked of them; if one of them is a business feature, its rules are missing. To put \
                 one back, add its slug to `keep_features` in the `[rules]` section of \
                 `retrodoc.toml` (that feature alone is asked again):\n\n",
                aside.features.len()
            );
            for AsideFeature {
                slug,
                name,
                description,
                reason,
            } in aside.features.iter().take(MAX_LISTED_RULES)
            {
                let _ = writeln!(
                    md,
                    "- `{slug}` {}: {} ({})",
                    one_line(name),
                    one_line(description),
                    one_line(reason)
                );
            }
            if aside.features.len() > MAX_LISTED_RULES {
                let _ = writeln!(
                    md,
                    "- … and {} more",
                    aside.features.len() - MAX_LISTED_RULES
                );
            }
        }
        if !aside.rules.is_empty() {
            let _ = write!(
                md,
                "\n### {} rule(s) dropped as noise\n\nTo put one back, add its slug to `keep_rules` \
                 in the `[rules]` section of `retrodoc.toml` (no LLM call):\n\n",
                aside.rules.len()
            );
            for AsideRule {
                slug,
                statement,
                class,
                ..
            } in aside.rules.iter().take(MAX_LISTED_RULES)
            {
                let mark = if self.rules_put_back.contains(slug) {
                    ", put back by hand"
                } else {
                    ""
                };
                let _ = writeln!(md, "- `{slug}` ({class}{mark}) {}", one_line(statement));
            }
            if aside.rules.len() > MAX_LISTED_RULES {
                let _ = writeln!(md, "- … and {} more", aside.rules.len() - MAX_LISTED_RULES);
            }
        }
    }
}
