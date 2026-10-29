//! Markdown/Mermaid rendering of the domain model (PLAN.md §3 layout):
//!
//! ```text
//! functional/llms.txt
//! functional/<domain>/README.md
//! functional/<domain>/<feature>.md
//! functional/<domain>/use-cases/<feature>/<use-case>.md
//! _retrodoc/coverage-report.md
//! _retrodoc/run-metadata.json
//! ```
//!
//! Use cases live in a folder per feature because use case slugs are only
//! unique within their feature.

use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use retrodoc_core::model::{
    BusinessRule, ConfidenceScore, Domain, Feature, Step, UseCase, LOW_CONFIDENCE_THRESHOLD,
};

use crate::doc_links::{DocLinks, ItemRef};
use crate::rules::{domain_section, index_page, rules_section};
use crate::{
    RenderedFile, RunMetadata, AGENT_INDEX_PATH, COVERAGE_REPORT_PATH, FUNCTIONAL_DIR,
    METADATA_PATH, RULES_INDEX_PATH,
};

/// Where the docs put what is not the core of the business: back-office tools, and use cases
/// that read as code or are weakly backed by it.
const TECHNICAL_HEADING: &str = "Administration and technical";
const TECHNICAL_NOTE: &str =
    "Back-office tools, and behaviours that read as code or that the code backs weakly.";

/// Renders every file of the docs, paths relative to the docs dir. Pure and
/// deterministic: the same artifacts always give the same bytes (the run
/// metadata's timestamp aside).
///
/// # Errors
///
/// Returns an error if the run metadata can't be serialized.
pub fn render(
    domains: &[Domain],
    features: &[Feature],
    use_cases: &[UseCase],
    coverage_report: &str,
    metadata: &RunMetadata,
) -> Result<Vec<RenderedFile>, crate::RenderError> {
    render_with_rules(domains, features, use_cases, &[], coverage_report, metadata)
}

/// [`render`] with the business rules: on the pages of their feature, use cases and domain, and in
/// an index by entity. Without rules the files are the same as [`render`]'s.
///
/// # Errors
///
/// Returns an error if the run metadata can't be serialized.
pub fn render_with_rules(
    domains: &[Domain],
    features: &[Feature],
    use_cases: &[UseCase],
    rules: &[BusinessRule],
    coverage_report: &str,
    metadata: &RunMetadata,
) -> Result<Vec<RenderedFile>, crate::RenderError> {
    render_with_docs(
        domains,
        features,
        use_cases,
        rules,
        &DocLinks::default(),
        coverage_report,
        metadata,
    )
}

/// [`render_with_rules`] with the sections of the project's own docs that document or contradict
/// each item, linked from its page (a rule: under it). Without links the files are the same.
///
/// # Errors
///
/// Returns an error if the run metadata can't be serialized.
pub fn render_with_docs(
    domains: &[Domain],
    features: &[Feature],
    use_cases: &[UseCase],
    rules: &[BusinessRule],
    docs: &DocLinks,
    coverage_report: &str,
    metadata: &RunMetadata,
) -> Result<Vec<RenderedFile>, crate::RenderError> {
    // A rule of a feature that no longer exists would be on no page.
    let rules: Vec<BusinessRule> = rules
        .iter()
        .filter(|r| features.iter().any(|f| r.belongs_to_feature(&f.slug)))
        .cloned()
        .collect();
    let rules = rules.as_slice();
    let mut files = Vec::new();

    for domain in domains {
        let domain_features: Vec<&Feature> = features
            .iter()
            .filter(|f| f.domain_slug == domain.slug)
            .collect();
        files.push(RenderedFile {
            path: domain_readme_path(&domain.slug),
            content: domain_page(domain, &domain_features, rules, docs),
        });
        for feature in domain_features {
            let own: Vec<&UseCase> = use_cases
                .iter()
                .filter(|u| u.feature_slug == feature.slug)
                .collect();
            let own_rules: Vec<&BusinessRule> = rules
                .iter()
                .filter(|r| r.belongs_to_feature(&feature.slug))
                .collect();
            files.push(RenderedFile {
                path: feature_path(feature),
                content: feature_page(feature, &own, &own_rules, docs),
            });
            for use_case in &own {
                let applying: Vec<&BusinessRule> = own_rules
                    .iter()
                    .copied()
                    .filter(|r| r.use_cases.contains(&use_case.slug))
                    .collect();
                files.push(RenderedFile {
                    path: use_case_path(feature, use_case),
                    content: use_case_page(feature, use_case, &applying, docs),
                });
            }
        }
    }

    if !rules.is_empty() {
        files.push(RenderedFile {
            path: PathBuf::from(RULES_INDEX_PATH),
            content: index_page(rules, features, Path::new(RULES_INDEX_PATH)),
        });
    }
    files.push(RenderedFile {
        path: PathBuf::from(AGENT_INDEX_PATH),
        content: agent_index(domains, features, rules.len()),
    });
    files.push(RenderedFile {
        path: PathBuf::from(COVERAGE_REPORT_PATH),
        content: coverage_report.to_string(),
    });
    files.push(RenderedFile {
        path: PathBuf::from(METADATA_PATH),
        content: format!("{}\n", serde_json::to_string_pretty(metadata)?),
    });
    Ok(files)
}

/// Slugs come from an LLM: keep a path segment to `[a-z0-9_-]` so that a
/// slug can never climb out of the docs dir.
fn segment(slug: &str) -> String {
    let cleaned: String = slug
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-');
    if cleaned.is_empty() {
        "unnamed".to_string()
    } else {
        cleaned.to_string()
    }
}

fn domain_dir(domain_slug: &str) -> PathBuf {
    Path::new(FUNCTIONAL_DIR).join(segment(domain_slug))
}

fn domain_readme_path(domain_slug: &str) -> PathBuf {
    domain_dir(domain_slug).join("README.md")
}

pub(crate) fn feature_path(feature: &Feature) -> PathBuf {
    domain_dir(&feature.domain_slug).join(format!("{}.md", segment(&feature.slug)))
}

pub(crate) fn use_case_path(feature: &Feature, use_case: &UseCase) -> PathBuf {
    domain_dir(&feature.domain_slug)
        .join("use-cases")
        .join(segment(&feature.slug))
        .join(format!("{}.md", segment(&use_case.slug)))
}

/// Relative Markdown link target from the file `from` to the file `to`.
pub(crate) fn link(from: &Path, to: &Path) -> String {
    let from_dir: Vec<Component> = from
        .parent()
        .unwrap_or(Path::new(""))
        .components()
        .collect();
    let to: Vec<Component> = to.components().collect();
    let common = from_dir.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_string(); from_dir.len() - common];
    parts.extend(
        to[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

fn confidence_line(score: Option<&ConfidenceScore>) -> String {
    score_line("Confidence", score)
}

/// `**<label>:** ⚠️ 40% — why`, the flag when below the threshold.
fn score_line(label: &str, score: Option<&ConfidenceScore>) -> String {
    let Some(score) = score else {
        return format!("**{label}:** not scored");
    };
    let flag = if score.value < LOW_CONFIDENCE_THRESHOLD {
        "⚠️ "
    } else {
        ""
    };
    let mut line = format!("**{label}:** {flag}{:.0}%", score.value * 100.0);
    if let Some(why) = score.rationale.as_deref().filter(|r| !r.trim().is_empty()) {
        let _ = write!(
            line,
            " — {}",
            why.split_whitespace().collect::<Vec<_>>().join(" ")
        );
    }
    line
}

fn short_confidence(score: Option<&ConfidenceScore>) -> String {
    score.map_or_else(
        || "not scored".to_string(),
        |c| format!("{:.0}%", c.value * 100.0),
    )
}

/// The `llms.txt`-style entry point for agents that don't run the MCP server.
fn agent_index(domains: &[Domain], features: &[Feature], rule_count: usize) -> String {
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

fn domain_page(
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

fn feature_page(
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

fn use_case_page(
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

fn step_item(step: &Step) -> String {
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

/// Exactly one trailing newline.
fn trim_end(md: &str) -> String {
    format!("{}\n", md.trim_end())
}

#[cfg(test)]
mod tests;
