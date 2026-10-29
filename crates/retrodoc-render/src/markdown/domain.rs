//! The page of a domain, `functional/<domain>/README.md`.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Domain, Feature};

use super::{
    confidence_line, domain_readme_path, feature_path, link, short_confidence, trim_end,
    TECHNICAL_HEADING, TECHNICAL_NOTE,
};
use crate::doc_links::{DocLinks, ItemRef};
use crate::rules::domain_section;

pub(super) fn domain_page(
    domain: &Domain,
    features: &[&Feature],
    rules: &[BusinessRule],
    docs: &DocLinks,
) -> String {
    let readme = domain_readme_path(&domain.slug);
    let mut md = format!("# {}\n\n{}\n\n", domain.name, domain.description.trim());
    let _ = writeln!(md, "{}\n", confidence_line(domain.confidence.as_ref()));

    let feature_item = |md: &mut String, f: &Feature| {
        let _ = writeln!(
            md,
            "- [{}]({}) ({}): {}",
            f.name,
            link(&readme, &feature_path(f)),
            short_confidence(f.confidence.as_ref()),
            f.description.trim()
        );
    };

    let business: Vec<&&Feature> = features.iter().filter(|f| !f.back_office).collect();
    if !business.is_empty() {
        md.push_str("## Features\n\n");
        for f in business {
            feature_item(&mut md, f);
        }
        md.push('\n');
    }

    if features.iter().any(|f| f.back_office) {
        let _ = write!(md, "## {TECHNICAL_HEADING}\n\n{TECHNICAL_NOTE}\n\n");
        for f in features.iter().filter(|f| f.back_office) {
            feature_item(&mut md, f);
        }
        md.push('\n');
    }
    let own: Vec<(&Feature, &BusinessRule)> = features
        .iter()
        .flat_map(|f| {
            rules
                .iter()
                .filter(|r| r.belongs_to_feature(&f.slug))
                .map(move |r| (*f, r))
        })
        .collect();
    md.push_str(&domain_section(&own, &readme));
    md.push_str(&docs.section(&ItemRef::Domain(domain.slug.clone()), &readme));
    trim_end(&md)
}
