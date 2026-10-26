//! The business rules in the docs (ADR 0021): on the page of the feature they belong to, on the
//! pages of the use cases they apply to, in the page of their domain, and in an index by entity
//! (`functional/rules.md`; there is no page per entity). Pure and deterministic, like the rest.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{
    BusinessRule, Feature, RuleEvidence, RuleKind, UseCase, LOW_CONFIDENCE_THRESHOLD,
};

use crate::doc_links::DocLinks;
use crate::markdown::{feature_path, link, use_case_path};

fn kind_label(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Validation => "Validation",
        RuleKind::Permission => "Permission",
        RuleKind::StateTransition => "State transition",
        RuleKind::Limit => "Limit",
        RuleKind::Computation => "Computation",
    }
}

/// `Limit, ⚠️ 40%`: the kind and the confidence, flagged when below the threshold (a rule backed
/// by code alone is a hypothesis).
fn tag(rule: &BusinessRule) -> String {
    match &rule.confidence {
        Some(c) if c.value < LOW_CONFIDENCE_THRESHOLD => {
            format!("{}, ⚠️ {:.0}%", kind_label(rule.kind), c.value * 100.0)
        }
        Some(c) => format!("{}, {:.0}%", kind_label(rule.kind), c.value * 100.0),
        None => format!("{}, not scored", kind_label(rule.kind)),
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn evidence_line(evidence: &RuleEvidence) -> String {
    match evidence {
        RuleEvidence::Code { source } => match (source.start_line, source.end_line) {
            (Some(s), Some(e)) if s != e => format!("Code: `{}:{s}-{e}`", source.path),
            (Some(s), _) => format!("Code: `{}:{s}`", source.path),
            _ => format!("Code: `{}`", source.path),
        },
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

/// The rules of a feature or of a use case, with their evidence. On a feature page, `use_cases`
/// are the feature's, and a rule links to those it applies to.
pub(crate) fn rules_section(
    rules: &[&BusinessRule],
    from: &Path,
    feature: &Feature,
    use_cases: &[&UseCase],
    docs: &DocLinks,
) -> String {
    if rules.is_empty() {
        return String::new();
    }
    let mut md = String::from("## Business rules\n\n");
    for rule in rules {
        let _ = writeln!(md, "- {} ({})", one_line(&rule.statement), tag(rule));
        if !rule.entities.is_empty() {
            let _ = writeln!(md, "  - Entities: {}", rule.entities.join(", "));
        }
        let applies: Vec<String> = use_cases
            .iter()
            .filter(|u| rule.use_cases.contains(&u.slug))
            .map(|u| format!("[{}]({})", u.name, link(from, &use_case_path(feature, u))))
            .collect();
        if !applies.is_empty() {
            let _ = writeln!(md, "  - Use cases: {}", applies.join(", "));
        }
        for evidence in &rule.evidence {
            let _ = writeln!(md, "  - {}", evidence_line(evidence));
        }
        md.push_str(&docs.rule_lines(&rule.slug, from));
    }
    md.push('\n');
    md
}

/// The rules of a domain, each with the feature it belongs to.
pub(crate) fn domain_section(rules: &[(&Feature, &BusinessRule)], from: &Path) -> String {
    if rules.is_empty() {
        return String::new();
    }
    let mut md = String::from("## Business rules\n\n");
    for (feature, rule) in rules {
        let _ = writeln!(
            md,
            "- {} ({}) — [{}]({})",
            one_line(&rule.statement),
            tag(rule),
            feature.name,
            link(from, &feature_path(feature))
        );
    }
    md.push('\n');
    md
}

/// All the rules grouped by entity (a rule on several entities appears under each; those on none
/// under "Other rules"), each linking to its feature.
pub(crate) fn index_page(rules: &[BusinessRule], features: &[Feature], from: &Path) -> String {
    let mut by_entity: BTreeMap<String, Vec<&BusinessRule>> = BTreeMap::new();
    for rule in rules {
        if rule.entities.is_empty() {
            by_entity.entry(String::new()).or_default().push(rule);
        }
        for entity in &rule.entities {
            by_entity.entry(entity.clone()).or_default().push(rule);
        }
    }
    let mut md = format!(
        "# Business rules\n\n> What is allowed, refused, limited and in which states, by entity: \
         {} rule(s) stated from the code, tests and documentation. A rule backed by code alone \
         (⚠️) is a hypothesis to check against the code.\n\n",
        rules.len()
    );
    // The entities in name order, the rules on none last.
    let groups = by_entity
        .iter()
        .filter(|(entity, _)| !entity.is_empty())
        .chain(by_entity.get_key_value(""));
    for (entity, own) in groups {
        let _ = write!(
            md,
            "## {}\n\n",
            if entity.is_empty() {
                "Other rules"
            } else {
                entity
            }
        );
        for rule in own {
            let feature = features.iter().find(|f| rule.belongs_to_feature(&f.slug));
            let place = feature.map_or_else(String::new, |f| {
                format!(" — [{}]({})", f.name, link(from, &feature_path(f)))
            });
            let _ = writeln!(md, "- {} ({}){place}", one_line(&rule.statement), tag(rule));
        }
        md.push('\n');
    }
    format!("{}\n", md.trim_end())
}
