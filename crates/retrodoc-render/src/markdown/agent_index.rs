//! The agent index, `functional/llms.txt`.

use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{Domain, Feature};

use super::{domain_readme_path, feature_path, link, list_item};
use crate::texts::Texts;
use crate::{AGENT_INDEX_PATH, RULES_INDEX_PATH};

/// The `llms.txt`-style entry point for agents that don't run the MCP server.
pub(super) fn agent_index(
    domains: &[Domain],
    features: &[Feature],
    rule_count: usize,
    texts: &Texts,
) -> String {
    let index = Path::new(AGENT_INDEX_PATH);
    let feature_item = |md: &mut String, indent: &str, f: &Feature| {
        list_item(
            md,
            indent,
            &f.name,
            &link(index, &feature_path(f)),
            f.confidence.as_ref(),
            &f.description,
            texts,
        );
    };

    let mut md = texts.agent_index_head.to_string();
    if domains.is_empty() {
        let _ = writeln!(md, "{}", texts.no_domain);
    }
    for domain in domains {
        list_item(
            &mut md,
            "",
            &domain.name,
            &link(index, &domain_readme_path(&domain.slug)),
            domain.confidence.as_ref(),
            &domain.description,
            texts,
        );
        let own: Vec<&Feature> = features
            .iter()
            .filter(|f| f.domain_slug == domain.slug)
            .collect();
        for f in own.iter().filter(|f| !f.back_office) {
            feature_item(&mut md, "  ", f);
        }
        if own.iter().any(|f| f.back_office) {
            let _ = writeln!(md, "  - {}:", texts.technical_heading);
            for f in own.iter().filter(|f| f.back_office) {
                feature_item(&mut md, "    ", f);
            }
        }
    }
    if rule_count > 0 {
        md.push_str(&(texts.agent_index_rules)(
            &link(index, Path::new(RULES_INDEX_PATH)),
            rule_count,
        ));
    }
    md.push_str(texts.agent_index_optional);
    md
}
