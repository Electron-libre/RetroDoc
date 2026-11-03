//! The page of a domain, `functional/<domain>/README.md`.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Domain, Feature};

use super::{confidence_line, domain_readme_path, feature_path, link, list_item, trim_end};
use crate::doc_links::{DocLinks, ItemRef};
use crate::rules::domain_section;
use crate::texts::Texts;

pub(super) fn domain_page(
    domain: &Domain,
    features: &[&Feature],
    rules: &[&BusinessRule],
    docs: &DocLinks,
    texts: &Texts,
) -> String {
    let readme = domain_readme_path(&domain.slug);
    let mut md = format!("# {}\n\n{}\n\n", domain.name, domain.description.trim());
    let _ = writeln!(
        md,
        "{}\n",
        confidence_line(domain.confidence.as_ref(), texts)
    );

    let feature_item = |md: &mut String, f: &Feature| {
        list_item(
            md,
            "",
            &f.name,
            &link(&readme, &feature_path(f)),
            f.confidence.as_ref(),
            &f.description,
            texts,
        );
    };

    let business: Vec<&&Feature> = features.iter().filter(|f| !f.back_office).collect();
    if !business.is_empty() {
        let _ = write!(md, "## {}\n\n", texts.features);
        for f in business {
            feature_item(&mut md, f);
        }
        md.push('\n');
    }

    if features.iter().any(|f| f.back_office) {
        let _ = write!(
            md,
            "## {}\n\n{}\n\n",
            texts.technical_heading, texts.technical_note
        );
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
                .map(move |r| (*f, *r))
        })
        .collect();
    md.push_str(&domain_section(&own, &readme, texts));
    md.push_str(&docs.section(&ItemRef::Domain(domain.slug.clone()), &readme, texts));
    trim_end(&md)
}
