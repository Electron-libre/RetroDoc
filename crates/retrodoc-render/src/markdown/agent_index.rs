//! The agent index, `functional/llms.txt`.

use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{ConfidenceScore, Domain, Feature, LOW_CONFIDENCE_THRESHOLD};

use super::{domain_readme_path, feature_path, link, short_confidence, TECHNICAL_HEADING};
use crate::{AGENT_INDEX_PATH, RULES_INDEX_PATH};

/// The `llms.txt`-style entry point for agents that don't run the MCP server.
pub(super) fn agent_index(domains: &[Domain], features: &[Feature], rule_count: usize) -> String {
    let index = Path::new(AGENT_INDEX_PATH);
    let one_line = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    // `(⚠️ 30%)` below the threshold, so a weak page is visible before opening it.
    let score = |score: Option<&ConfidenceScore>| match score {
        Some(c) if c.value < LOW_CONFIDENCE_THRESHOLD => format!("⚠️ {:.0}%", c.value * 100.0),
        other => short_confidence(other),
    };
    let feature_item = |md: &mut String, indent: &str, f: &Feature| {
        let _ = writeln!(
            md,
            "{indent}- [{}]({}) ({}): {}",
            f.name,
            link(index, &feature_path(f)),
            score(f.confidence.as_ref()),
            one_line(&f.description)
        );
    };

    let mut md = String::from(
        "# Functional documentation\n\n\
         > What the application does, in business terms: domains, features and use cases derived \
         from its code and history by RetroDoc. Every page carries a confidence score; below 50% \
         (⚠️) the page is not backed by the code and must be checked against it.\n\n\
         ## How to read it\n\n\
         - Go from a domain to its features, then to their use cases: each page links to the next \
         level and lists the source files it rests on.\n\
         - The pages are generated and date from the last `retrodoc generate` run (see the \
         [run metadata](../_retrodoc/run-metadata.json)): check the cited files if the code has \
         changed since.\n\
         - An agent that can run commands gets the same documentation, with a warning when the \
         cited code changed, from the MCP server: `retrodoc mcp --path <repo>`.\n\n\
         ## Domains\n\n",
    );
    if domains.is_empty() {
        md.push_str("No domain documented yet.\n");
    }
    for domain in domains {
        let _ = writeln!(
            md,
            "- [{}]({}) ({}): {}",
            domain.name,
            link(index, &domain_readme_path(&domain.slug)),
            score(domain.confidence.as_ref()),
            one_line(&domain.description)
        );
        let own: Vec<&Feature> = features
            .iter()
            .filter(|f| f.domain_slug == domain.slug)
            .collect();
        for f in own.iter().filter(|f| !f.back_office) {
            feature_item(&mut md, "  ", f);
        }
        if own.iter().any(|f| f.back_office) {
            let _ = writeln!(md, "  - {TECHNICAL_HEADING}:");
            for f in own.iter().filter(|f| f.back_office) {
                feature_item(&mut md, "    ", f);
            }
        }
    }
    if rule_count > 0 {
        let _ = write!(
            md,
            "\n## Business rules\n\n- [Business rules by entity]({}): {rule_count} rule(s) on what is \
             allowed, refused, limited and in which states, each with its evidence; they are also on \
             the pages of their feature, use cases and domain\n",
            link(index, Path::new(RULES_INDEX_PATH))
        );
    }
    md.push_str(
        "\n## Optional\n\n\
         - [Documentation debt report](../_retrodoc/coverage-report.md): the sections with a low \
         confidence and the code that no domain covers\n",
    );
    md
}
