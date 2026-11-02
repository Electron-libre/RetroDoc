//! The page of a feature, `functional/<domain>/<feature>.md`.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Feature, UseCase};

use super::{
    confidence_line, domain_readme_path, feature_path, link, short_confidence, trim_end,
    use_case_path,
};
use crate::doc_links::{DocLinks, ItemRef};
use crate::rules::rules_section;
use crate::texts::Texts;

pub(super) fn feature_page(
    feature: &Feature,
    use_cases: &[&UseCase],
    rules: &[&BusinessRule],
    docs: &DocLinks,
    texts: &Texts,
) -> String {
    let path = feature_path(feature);
    let mut md = format!("# {}\n\n", feature.name);
    let _ = writeln!(
        md,
        "[← {}]({})\n",
        texts.domain_overview,
        link(&path, &domain_readme_path(&feature.domain_slug))
    );
    let _ = writeln!(md, "{}\n", feature.description.trim());
    let _ = writeln!(
        md,
        "{}\n",
        confidence_line(feature.confidence.as_ref(), texts)
    );

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
            short_confidence(u.confidence.as_ref(), texts),
            u.description.trim()
        );
    };
    let _ = write!(md, "## {}\n\n", texts.use_cases);
    if use_cases.is_empty() {
        let _ = writeln!(md, "{}", texts.no_use_case);
    } else if business.is_empty() {
        let _ = writeln!(md, "{}", texts.all_set_apart);
    }
    for u in &business {
        use_case_item(&mut md, u);
    }
    if !technical.is_empty() {
        let _ = write!(
            md,
            "\n## {}\n\n{}\n\n",
            texts.technical_heading, texts.technical_note
        );
        for u in &technical {
            use_case_item(&mut md, u);
        }
    }

    if !rules.is_empty() {
        md.push('\n');
        md.push_str(&rules_section(
            rules, &path, feature, use_cases, docs, texts,
        ));
    }
    let existing = docs.section(&ItemRef::Feature(feature.slug.clone()), &path, texts);
    if !existing.is_empty() {
        md.push('\n');
        md.push_str(&existing);
    }

    if !feature.source_paths.is_empty() {
        let _ = write!(md, "\n## {}\n\n", texts.source_files);
        for p in &feature.source_paths {
            let _ = writeln!(md, "- `{p}`");
        }
    }
    trim_end(&md)
}
