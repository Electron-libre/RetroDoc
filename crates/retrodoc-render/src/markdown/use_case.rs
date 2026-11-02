//! The page of a use case, `functional/<domain>/use-cases/<feature>/<use-case>.md`.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Feature, Step, UseCase};

use super::{confidence_line, feature_path, link, score_line, trim_end, use_case_path};
use crate::doc_links::{DocLinks, ItemRef};
use crate::rules::rules_section;
use crate::texts::Texts;

pub(super) fn use_case_page(
    feature: &Feature,
    use_case: &UseCase,
    rules: &[&BusinessRule],
    docs: &DocLinks,
    texts: &Texts,
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
    let _ = writeln!(
        md,
        "{}\n",
        confidence_line(use_case.confidence.as_ref(), texts)
    );
    if use_case.business_language.is_some() {
        let _ = writeln!(
            md,
            "{}\n",
            score_line(
                texts.business_language,
                use_case.business_language.as_ref(),
                texts
            )
        );
    }
    if let Some(actor) = &use_case.primary_actor {
        let _ = writeln!(md, "**{}** {actor}\n", texts.primary_actor);
    }
    if !use_case.entry_points.is_empty() {
        let names: Vec<String> = use_case
            .entry_point_names()
            .iter()
            .map(|n| format!("`{n}`"))
            .collect();
        let _ = writeln!(md, "**{}** {}\n", texts.triggered_by, names.join(", "));
    }

    // Two levels: the business narrative first, the technical steps and
    // diagram folded below it. Without a narrative (older artifacts) the
    // steps stay in the open, as they were.
    let narrative = use_case
        .narrative
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    let level = if let Some(narrative) = narrative {
        let _ = write!(md, "## {}\n\n{narrative}\n\n", texts.what_happens);
        md.push_str(&rules_section(rules, &path, feature, &[], docs, texts));
        let _ = write!(
            md,
            "<details>\n<summary>{}</summary>\n\n",
            (texts.technical_details)(use_case.steps.len())
        );
        "###"
    } else {
        md.push_str(&rules_section(rules, &path, feature, &[], docs, texts));
        "##"
    };

    let _ = write!(md, "{level} {}\n\n", texts.steps);
    for step in &use_case.steps {
        md.push_str(&step_item(step, texts));
    }

    if let Some(diagram) = &use_case.diagram_mermaid {
        let _ = write!(
            md,
            "\n{level} {}\n\n```mermaid\n{}\n```\n",
            texts.diagram,
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
        texts,
    );
    if !existing.is_empty() {
        md.push('\n');
        md.push_str(&existing);
    }
    trim_end(&md)
}

pub(super) fn step_item(step: &Step, texts: &Texts) -> String {
    let kind = if step.actor.kind == retrodoc_core::model::ActorKind::Human {
        texts.human
    } else {
        texts.system
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
            .map(|r| format!("`{}`", super::source_ref_label(r)))
            .collect();
        let _ = writeln!(item, "   - {} {}", texts.code, refs.join(", "));
    }
    item
}
