//! The page of a feature, `functional/<domain>/<feature>.md`.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Feature, UseCase};

use super::{
    confidence_line, domain_readme_path, feature_path, link, short_confidence, trim_end,
    use_case_path, TECHNICAL_HEADING, TECHNICAL_NOTE,
};
use crate::doc_links::{DocLinks, ItemRef};
use crate::rules::rules_section;

pub(super) fn feature_page(
    feature: &Feature,
    use_cases: &[&UseCase],
    rules: &[&BusinessRule],
    docs: &DocLinks,
) -> String {
    let path = feature_path(feature);
    let mut md = format!("# {}\n\n", feature.name);
    let _ = writeln!(
        md,
        "[← Domain overview]({})\n",
        link(&path, &domain_readme_path(&feature.domain_slug))
    );
    let _ = writeln!(md, "{}\n", feature.description.trim());
    let _ = writeln!(md, "{}\n", confidence_line(feature.confidence.as_ref()));

    let (technical, business): (Vec<&UseCase>, Vec<&UseCase>) = use_cases
        .iter()
        .copied()
        .partition(|u| u.is_technical(feature.back_office));
    let use_case_item = |md: &mut String, u: &UseCase| {
        let _ = writeln!(
            md,
            "- [{}]({}) ({}): {}",
            u.name,
            link(&path, &use_case_path(feature, u)),
            short_confidence(u.confidence.as_ref()),
            u.description.trim()
        );
    };
    md.push_str("## Use cases\n\n");
    if use_cases.is_empty() {
        md.push_str("None could be derived from the code.\n");
    } else if business.is_empty() {
        md.push_str("All of them are set apart below.\n");
    }
    for u in &business {
        use_case_item(&mut md, u);
    }
    if !technical.is_empty() {
        let _ = write!(md, "\n## {TECHNICAL_HEADING}\n\n{TECHNICAL_NOTE}\n\n");
        for u in &technical {
            use_case_item(&mut md, u);
        }
    }

    if !rules.is_empty() {
        md.push('\n');
        md.push_str(&rules_section(rules, &path, feature, use_cases, docs));
    }
    let existing = docs.section(&ItemRef::Feature(feature.slug.clone()), &path);
    if !existing.is_empty() {
        md.push('\n');
        md.push_str(&existing);
    }

    if !feature.source_paths.is_empty() {
        md.push_str("\n## Source files\n\n");
        for p in &feature.source_paths {
            let _ = writeln!(md, "- `{p}`");
        }
    }
    trim_end(&md)
}
