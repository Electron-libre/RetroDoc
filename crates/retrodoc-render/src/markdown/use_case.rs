//! The page of a use case, `functional/<domain>/use-cases/<feature>/<use-case>.md`.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Feature, Step, UseCase};

use super::{confidence_line, feature_path, link, score_line, trim_end, use_case_path};
use crate::doc_links::{DocLinks, ItemRef};
use crate::rules::rules_section;

pub(super) fn use_case_page(
    feature: &Feature,
    use_case: &UseCase,
    rules: &[&BusinessRule],
    docs: &DocLinks,
) -> String {
    let path = use_case_path(feature, use_case);
    let mut md = format!("# {}\n\n", use_case.name);
    let _ = writeln!(
        md,
        "[← {}]({})\n",
        feature.name,
        link(&path, &feature_path(feature))
    );
    let _ = writeln!(md, "{}\n", use_case.description.trim());
    let _ = writeln!(md, "{}\n", confidence_line(use_case.confidence.as_ref()));
    if use_case.business_language.is_some() {
        let _ = writeln!(
            md,
            "{}\n",
            score_line("Business language", use_case.business_language.as_ref())
        );
    }
    if let Some(actor) = &use_case.primary_actor {
        let _ = writeln!(md, "**Primary actor:** {actor}\n");
    }
    if !use_case.entry_points.is_empty() {
        let names: Vec<String> = use_case
            .entry_point_names()
            .iter()
            .map(|n| format!("`{n}`"))
            .collect();
        let _ = writeln!(md, "**Triggered by:** {}\n", names.join(", "));
    }

    // Two levels: the business narrative first, the technical steps and
    // diagram folded below it. Without a narrative (older artifacts) the
    // steps stay in the open, as they were.
    let narrative = use_case
        .narrative
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    let (steps_heading, diagram_heading) = if let Some(narrative) = narrative {
        let _ = write!(md, "## What happens\n\n{narrative}\n\n");
        md.push_str(&rules_section(rules, &path, feature, &[], docs));
        let _ = write!(
            md,
            "<details>\n<summary>Technical details ({} step{})</summary>\n\n",
            use_case.steps.len(),
            if use_case.steps.len() == 1 { "" } else { "s" }
        );
        ("### Steps", "### Diagram")
    } else {
        md.push_str(&rules_section(rules, &path, feature, &[], docs));
        ("## Steps", "## Diagram")
    };

    let _ = write!(md, "{steps_heading}\n\n");
    for step in &use_case.steps {
        md.push_str(&step_item(step));
    }

    if let Some(diagram) = &use_case.diagram_mermaid {
        let _ = write!(
            md,
            "\n{diagram_heading}\n\n```mermaid\n{}\n```\n",
            diagram.trim_end()
        );
    }
    if narrative.is_some() {
        md.push_str("\n</details>\n");
    }
    let existing = docs.section(
        &ItemRef::UseCase {
            feature: feature.slug.clone(),
            use_case: use_case.slug.clone(),
        },
        &path,
    );
    if !existing.is_empty() {
        md.push('\n');
        md.push_str(&existing);
    }
    trim_end(&md)
}

pub(super) fn step_item(step: &Step) -> String {
    let kind = if step.actor.kind == retrodoc_core::model::ActorKind::Human {
        "human"
    } else {
        "system"
    };
    let mut item = format!(
        "{}. **{}** ({kind}) — {}",
        step.order,
        step.actor.name,
        step.label()
    );
    if let Some(detail) = step.detail() {
        let _ = write!(item, ". {detail}");
    }
    item.push('\n');
    if !step.source_refs.is_empty() {
        let refs: Vec<String> = step
            .source_refs
            .iter()
            .map(|r| match (r.start_line, r.end_line) {
                (Some(s), Some(e)) if s != e => format!("`{}:{s}-{e}`", r.path),
                (Some(s), _) => format!("`{}:{s}`", r.path),
                _ => format!("`{}`", r.path),
            })
            .collect();
        let _ = writeln!(item, "   - Code: {}", refs.join(", "));
    }
    item
}
