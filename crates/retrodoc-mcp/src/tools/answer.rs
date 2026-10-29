//! The pieces the answers of the tools share: confidence, rules, stale warnings, set-apart
//! marks, and the answer to an unknown id.

use std::fmt::Write as _;

use retrodoc_core::model::{
    BusinessRule, ConfidenceScore, RuleEvidence, RuleKind, SourceRef, UseCase,
    LOW_CONFIDENCE_THRESHOLD,
};

use super::Docs;
use crate::corpus::{feature_id, Entry, EntryKind};
use crate::freshness::FileState;

impl Docs {
    /// Whether the docs set a use case apart under "Administration and technical".
    pub(super) fn use_case_set_apart(&self, use_case: &UseCase) -> bool {
        let back_office = self
            .features
            .iter()
            .any(|f| f.slug == use_case.feature_slug && f.back_office);
        use_case.is_technical(back_office)
    }

    /// Whether a search entry is a back-office feature or a use case set apart.
    pub(super) fn entry_set_apart(&self, entry: &Entry) -> bool {
        match entry.kind {
            EntryKind::Feature => self
                .features
                .iter()
                .any(|f| f.back_office && feature_id(f) == entry.id),
            EntryKind::UseCase => self.use_cases.iter().any(|u| {
                self.features.iter().any(|f| {
                    f.slug == u.feature_slug && format!("{}/{}", feature_id(f), u.slug) == entry.id
                }) && self.use_case_set_apart(u)
            }),
            _ => false,
        }
    }

    /// The `## Business rules` section of a feature or a use case: each rule with its id, kind,
    /// confidence and evidence, and a stale warning for the code it cites. Empty without rules.
    pub(super) fn rules_section(&self, rules: &[&BusinessRule]) -> String {
        if rules.is_empty() {
            return String::new();
        }
        let mut out = String::from("\n## Business rules\n\n");
        for rule in rules {
            let _ = writeln!(out, "{}", rule_item(rule));
            if !rule.entities.is_empty() {
                let _ = writeln!(out, "  - Entities: {}", rule.entities.join(", "));
            }
            for evidence in &rule.evidence {
                let _ = writeln!(out, "  - {}", evidence_line(evidence));
            }
            let code = rule.evidence.iter().filter_map(|e| match e {
                RuleEvidence::Code { source } => Some(source.path.as_str()),
                _ => None,
            });
            let stale = self.stale_files(code);
            if !stale.is_empty() {
                let _ = writeln!(out, "  - ⚠ stale: {}", stale.join(", "));
            }
        }
        out
    }

    /// The cited files that changed since generation, as `` `path` (modified) ``.
    pub(super) fn stale_files<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let Some(freshness) = &self.freshness else {
            return Vec::new();
        };
        let mut seen = Vec::new();
        let paths = paths.into_iter().filter(|p| {
            let new = !seen.contains(p);
            seen.push(p);
            new
        });
        freshness
            .stale(paths)
            .into_iter()
            .map(|(path, state)| {
                let what = if state == FileState::Deleted {
                    "deleted"
                } else {
                    "modified"
                };
                format!("`{path}` ({what})")
            })
            .collect()
    }

    /// A warning paragraph when code cited by an answer changed since the
    /// docs were generated; empty otherwise.
    pub(super) fn stale_notice<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> String {
        let stale = self.stale_files(paths);
        if stale.is_empty() {
            return String::new();
        }
        format!(
            "\n⚠ Stale: the code cited here changed since these docs were generated: {}. \
             Verify against the current code before relying on this.\n",
            stale.join(", ")
        )
    }

    pub(super) fn unknown(&self, what: &str, id: &str, browse_with: &str) -> String {
        let mut out = format!("Not documented: no {what} `{id}`.");
        if what == "domain" {
            let known: Vec<&str> = self.domains.iter().map(|d| d.slug.as_str()).collect();
            let _ = write!(out, " Known domains: {}.", known.join(", "));
        }
        let _ = writeln!(
            out,
            " Use `{browse_with}` or `search_docs` to find the right id."
        );
        out
    }
}

pub(super) const NO_SOURCE_ACCESS: &str =
    "Not available: this server has no access to the repository's code.\n";

/// Characters of a description shown in a list or a search hit.
pub(super) const SNIPPET_CHARS: usize = 160;

/// What an answer says of a feature or use case the docs set apart under "Administration and
/// technical": an agent can still read it, and knows it is not the core of the business.
pub(super) const SET_APART_NOTE: &str = "Administration and technical: a back-office tool, or a behaviour that reads as code or that the code backs weakly (not the core of the business).";

pub(super) fn set_apart_tag(set_apart: bool) -> &'static str {
    if set_apart {
        " [administration/technical]"
    } else {
        ""
    }
}

pub(super) fn last_segment(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

pub(super) fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    format!("{cut}…")
}

pub(super) fn one_line(text: &str) -> String {
    truncate(
        text.lines().next().unwrap_or_default().trim(),
        SNIPPET_CHARS,
    )
}

pub(super) fn percent(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

/// `confidence 80%`, `confidence 30% (low)` or `not scored`.
pub(super) fn confidence_short(score: Option<&ConfidenceScore>) -> String {
    match score {
        None => "not scored".into(),
        Some(s) if s.value < LOW_CONFIDENCE_THRESHOLD => {
            format!("confidence {} (low)", percent(s.value))
        }
        Some(s) => format!("confidence {}", percent(s.value)),
    }
}

/// The full line of a detail answer, with the rationale and, when low, the
/// advice to check the code.
pub(super) fn confidence_line(score: Option<&ConfidenceScore>) -> String {
    let Some(score) = score else {
        return "Confidence: not scored — treat as unverified.".into();
    };
    let mut line = format!("Confidence: {}", percent(score.value));
    if let Some(rationale) = &score.rationale {
        let _ = write!(line, " ({rationale})");
    }
    if score.value < LOW_CONFIDENCE_THRESHOLD {
        line.push_str(" — low: verify against the code before relying on it.");
    }
    line
}

pub(super) fn sources_section<'a>(title: &str, paths: impl Iterator<Item = &'a str>) -> String {
    let paths: Vec<&str> = paths.collect();
    if paths.is_empty() {
        return String::new();
    }
    format!("\n{title}: {}\n", paths.join(", "))
}

pub(super) fn kind_label(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Validation => "validation",
        RuleKind::Permission => "permission",
        RuleKind::StateTransition => "state transition",
        RuleKind::Limit => "limit",
        RuleKind::Computation => "computation",
    }
}

/// `- `id` — statement (kind, confidence 85%)`; below the threshold the confidence reads `(low)`.
pub(super) fn rule_item(rule: &BusinessRule) -> String {
    format!(
        "- `{}` — {} ({}, {})",
        rule.slug,
        one_line(&rule.statement),
        kind_label(rule.kind),
        confidence_short(rule.confidence.as_ref())
    )
}

pub(super) fn evidence_line(evidence: &RuleEvidence) -> String {
    match evidence {
        RuleEvidence::Code { source } => format!("Code: {}", location(source)),
        RuleEvidence::Test { phrase, path } => {
            format!("Test: \"{}\" (`{path}`)", one_line(phrase))
        }
        RuleEvidence::Doc { path, heading } if heading.is_empty() => {
            format!("Documentation: `{path}`")
        }
        RuleEvidence::Doc { path, heading } => {
            format!("Documentation: `{path}`, section \"{}\"", one_line(heading))
        }
    }
}

pub(super) fn location(source: &SourceRef) -> String {
    match (source.start_line, source.end_line) {
        (Some(start), Some(end)) if start != end => format!("`{}:{start}-{end}`", source.path),
        (Some(line), _) => format!("`{}:{line}`", source.path),
        _ => format!("`{}`", source.path),
    }
}
