//! Markdown/Mermaid rendering of the domain model (PLAN.md §3 layout):
//!
//! ```text
//! functional/llms.txt
//! functional/<domain>/README.md
//! functional/<domain>[/<sub-domain>]/<feature>.md
//! functional/<domain>[/<sub-domain>]/use-cases/<feature>/<use-case>.md
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

use crate::rules::{domain_section, index_page, rules_section};
use crate::{
    RenderedFile, RunMetadata, AGENT_INDEX_PATH, COVERAGE_REPORT_PATH, FUNCTIONAL_DIR,
    METADATA_PATH, RULES_INDEX_PATH,
};

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
            content: domain_page(domain, &domain_features, rules),
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
                content: feature_page(feature, &own, &own_rules),
            });
            for use_case in &own {
                let applying: Vec<&BusinessRule> = own_rules
                    .iter()
                    .copied()
                    .filter(|r| r.use_cases.contains(&use_case.slug))
                    .collect();
                files.push(RenderedFile {
                    path: use_case_path(feature, use_case),
                    content: use_case_page(feature, use_case, &applying),
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

fn container(feature: &Feature) -> PathBuf {
    let dir = domain_dir(&feature.domain_slug);
    match &feature.sub_domain_slug {
        Some(sub) => dir.join(segment(sub)),
        None => dir,
    }
}

pub(crate) fn feature_path(feature: &Feature) -> PathBuf {
    container(feature).join(format!("{}.md", segment(&feature.slug)))
}

pub(crate) fn use_case_path(feature: &Feature, use_case: &UseCase) -> PathBuf {
    container(feature)
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
        let in_sub = |f: &Feature| {
            f.sub_domain_slug
                .as_ref()
                .is_some_and(|s| domain.sub_domains.iter().any(|d| &d.slug == s))
        };
        for f in own.iter().filter(|f| !in_sub(f)) {
            feature_item(&mut md, "  ", f);
        }
        for sub in &domain.sub_domains {
            let _ = writeln!(md, "  - {}: {}", sub.name, one_line(&sub.description));
            for f in own
                .iter()
                .filter(|f| f.sub_domain_slug.as_deref() == Some(sub.slug.as_str()))
            {
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

fn domain_page(domain: &Domain, features: &[&Feature], rules: &[BusinessRule]) -> String {
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

    let is_in_sub = |f: &&&Feature| {
        f.sub_domain_slug
            .as_ref()
            .is_some_and(|s| domain.sub_domains.iter().any(|d| &d.slug == s))
    };
    let direct: Vec<&&Feature> = features.iter().filter(|f| !is_in_sub(f)).collect();
    if !direct.is_empty() {
        md.push_str("## Features\n\n");
        for f in direct {
            feature_item(&mut md, f);
        }
        md.push('\n');
    }

    if !domain.sub_domains.is_empty() {
        md.push_str("## Sub-domains\n\n");
        for sub in &domain.sub_domains {
            let _ = write!(md, "### {}\n\n{}\n\n", sub.name, sub.description.trim());
            let _ = writeln!(md, "{}\n", confidence_line(sub.confidence.as_ref()));
            for f in features
                .iter()
                .filter(|f| f.sub_domain_slug.as_deref() == Some(sub.slug.as_str()))
            {
                feature_item(&mut md, f);
            }
            md.push('\n');
        }
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
    trim_end(&md)
}

fn feature_page(feature: &Feature, use_cases: &[&UseCase], rules: &[&BusinessRule]) -> String {
    let path = feature_path(feature);
    let mut md = format!("# {}\n\n", feature.name);
    let _ = writeln!(
        md,
        "[← Domain overview]({})\n",
        link(&path, &domain_readme_path(&feature.domain_slug))
    );
    let _ = writeln!(md, "{}\n", feature.description.trim());
    let _ = writeln!(md, "{}\n", confidence_line(feature.confidence.as_ref()));

    md.push_str("## Use cases\n\n");
    if use_cases.is_empty() {
        md.push_str("None could be derived from the code.\n");
    }
    for u in use_cases {
        let _ = writeln!(
            md,
            "- [{}]({}) ({}): {}",
            u.name,
            link(&path, &use_case_path(feature, u)),
            short_confidence(u.confidence.as_ref()),
            u.description.trim()
        );
    }

    if !rules.is_empty() {
        md.push('\n');
        md.push_str(&rules_section(rules, &path, feature, use_cases));
    }

    if !feature.source_paths.is_empty() {
        md.push_str("\n## Source files\n\n");
        for p in &feature.source_paths {
            let _ = writeln!(md, "- `{p}`");
        }
    }
    trim_end(&md)
}

fn use_case_page(feature: &Feature, use_case: &UseCase, rules: &[&BusinessRule]) -> String {
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
            .entry_points
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
        md.push_str(&rules_section(rules, &path, feature, &[]));
        let _ = write!(
            md,
            "<details>\n<summary>Technical details ({} step{})</summary>\n\n",
            use_case.steps.len(),
            if use_case.steps.len() == 1 { "" } else { "s" }
        );
        ("### Steps", "### Diagram")
    } else {
        md.push_str(&rules_section(rules, &path, feature, &[]));
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
        step.order, step.actor.name, step.action
    );
    let description = step.description.trim();
    if !description.is_empty() {
        let _ = write!(item, ". {description}");
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
mod tests {
    use super::*;

    use retrodoc_core::model::{Actor, ActorKind, SourceRef, SubDomain};

    fn meta() -> RunMetadata {
        RunMetadata {
            model: "m".to_string(),
            commit: Some("abc".to_string()),
            generated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn sample() -> (Vec<Domain>, Vec<Feature>, Vec<UseCase>) {
        let domains = vec![Domain {
            slug: "billing".to_string(),
            name: "Billing".to_string(),
            description: "Invoices.".to_string(),
            sub_domains: vec![SubDomain {
                slug: "payment".to_string(),
                name: "Payment".to_string(),
                description: "Paying.".to_string(),
                confidence: None,
            }],
            confidence: Some(ConfidenceScore::new(0.8, None)),
        }];
        let features = vec![
            Feature {
                slug: "pay-invoice".to_string(),
                domain_slug: "billing".to_string(),
                sub_domain_slug: Some("payment".to_string()),
                name: "Pay an invoice".to_string(),
                description: "Customers pay.".to_string(),
                source_paths: vec!["src/pay.rs".to_string()],
                entry_points: Vec::new(),
                confidence: Some(ConfidenceScore::new(0.3, "weak".to_string())),
            },
            Feature {
                slug: "list".to_string(),
                domain_slug: "billing".to_string(),
                sub_domain_slug: None,
                name: "List".to_string(),
                description: "Listing.".to_string(),
                source_paths: vec![],
                entry_points: Vec::new(),
                confidence: None,
            },
        ];
        let use_cases = vec![UseCase {
            entry_points: Vec::new(),
            primary_actor: None,
            narrative: None,
            business_language: None,
            slug: "pay-by-card".to_string(),
            feature_slug: "pay-invoice".to_string(),
            name: "Pay by card".to_string(),
            description: "Card flow.".to_string(),
            steps: vec![Step {
                order: 1,
                description: "Submit the form".to_string(),
                actor: Actor {
                    name: "Customer".to_string(),
                    kind: ActorKind::Human,
                },
                action: "submit".to_string(),
                source_refs: vec![SourceRef {
                    path: "src/pay.rs".to_string(),
                    start_line: Some(3),
                    end_line: Some(9),
                }],
            }],
            diagram_mermaid: Some("sequenceDiagram\n    A->>B: x".to_string()),
            confidence: Some(ConfidenceScore::new(0.3, "step 1: guessed".to_string())),
        }];
        (domains, features, use_cases)
    }

    #[test]
    fn renders_the_planned_layout_with_working_links() {
        let (d, f, u) = sample();
        let files = render(&d, &f, &u, "# report\n", &meta()).unwrap();
        let paths: Vec<String> = files.iter().map(|f| f.path.display().to_string()).collect();
        assert_eq!(
            paths,
            [
                "functional/billing/README.md",
                "functional/billing/payment/pay-invoice.md",
                "functional/billing/payment/use-cases/pay-invoice/pay-by-card.md",
                "functional/billing/list.md",
                "functional/llms.txt",
                "_retrodoc/coverage-report.md",
                "_retrodoc/run-metadata.json",
            ]
        );
        let content = |p: &str| {
            &files
                .iter()
                .find(|f| f.path == Path::new(p))
                .unwrap()
                .content
        };

        let readme = content("functional/billing/README.md");
        assert!(readme.contains("](payment/pay-invoice.md)"));
        assert!(readme.contains("](list.md)"));

        let feature = content("functional/billing/payment/pay-invoice.md");
        assert!(feature.contains("](../README.md)"));
        assert!(feature.contains("](use-cases/pay-invoice/pay-by-card.md)"));
        assert!(feature.contains("⚠️ 30% — weak"));

        let use_case = content("functional/billing/payment/use-cases/pay-invoice/pay-by-card.md");
        assert!(use_case.contains("](../../pay-invoice.md)"));
        assert!(use_case.contains("1. **Customer** (human) — submit. Submit the form"));
        assert!(use_case.contains("`src/pay.rs:3-9`"));
        assert!(use_case.contains("```mermaid\nsequenceDiagram"));
    }

    #[test]
    fn a_narrative_comes_first_and_folds_the_technical_steps() {
        let (d, f, mut u) = sample();
        u[0].narrative = Some("A customer pays an invoice by card.".to_string());
        let files = render(&d, &f, &u, "r", &meta()).unwrap();
        let page = &files
            .iter()
            .find(|f| f.path.ends_with("pay-by-card.md"))
            .unwrap()
            .content;

        let narrative = page.find("## What happens").unwrap();
        let details = page.find("<details>").unwrap();
        let step = page.find("1. **Customer**").unwrap();
        assert!(narrative < details && details < step);
        assert!(page.contains("A customer pays an invoice by card."));
        assert!(page.contains("<summary>Technical details (1 step)</summary>"));
        assert!(page.contains("### Steps") && page.contains("### Diagram"));
        assert!(page.trim_end().ends_with("</details>"));
        // The business-language score is shown next to the confidence, flagged when low.
        assert!(!page.contains("Business language"));
        u[0].business_language = Some(ConfidenceScore::new(
            0.3,
            "names no business entity".to_string(),
        ));
        let files = render(&d, &f, &u, "r", &meta()).unwrap();
        let scored = &files
            .iter()
            .find(|f| f.path.ends_with("pay-by-card.md"))
            .unwrap()
            .content;
        assert!(scored.contains("**Business language:** ⚠️ 30% — names no business entity"));
        // Without a narrative the steps stay in the open, unfolded.
        let (d, f, u) = sample();
        let files = render(&d, &f, &u, "r", &meta()).unwrap();
        let page = &files
            .iter()
            .find(|f| f.path.ends_with("pay-by-card.md"))
            .unwrap()
            .content;
        assert!(page.contains("## Steps") && !page.contains("<details>"));
    }

    fn index_of(files: &[RenderedFile]) -> &str {
        &files
            .iter()
            .find(|f| f.path == Path::new(AGENT_INDEX_PATH))
            .unwrap()
            .content
    }

    #[test]
    fn the_agent_index_lists_domains_and_features_with_links_and_confidence() {
        let (d, f, u) = sample();
        let files = render(&d, &f, &u, "r", &meta()).unwrap();
        let index = index_of(&files);
        assert!(index.starts_with("# Functional documentation\n"), "{index}");
        assert!(
            index.contains("- [Billing](billing/README.md) (80%): Invoices."),
            "{index}"
        );
        assert!(
            index.contains(
                "[Pay an invoice](billing/payment/pay-invoice.md) (⚠️ 30%): Customers pay."
            ),
            "{index}"
        );
        assert!(
            index.contains("[List](billing/list.md) (not scored): Listing."),
            "{index}"
        );
        assert!(
            index.contains("(../_retrodoc/coverage-report.md)"),
            "{index}"
        );
        assert!(index.contains("retrodoc mcp --path"), "{index}");
    }

    #[test]
    fn a_domain_without_feature_is_still_listed_and_no_domain_gives_a_clear_index() {
        let (d, _, _) = sample();
        let index = agent_index(&d, &[], 0);
        assert!(index.contains("- [Billing](billing/README.md)"), "{index}");
        let empty = agent_index(&[], &[], 0);
        assert!(empty.contains("No domain documented yet."), "{empty}");
    }

    #[test]
    fn a_rerun_on_unchanged_artifacts_rewrites_nothing_including_the_index() {
        let (d, f, u) = sample();
        let dir = tempfile::tempdir().unwrap();
        let first = crate::plan(dir.path(), render(&d, &f, &u, "r", &meta()).unwrap()).unwrap();
        first.apply(dir.path()).unwrap();
        assert!(dir.path().join(AGENT_INDEX_PATH).exists());
        let second = crate::plan(dir.path(), render(&d, &f, &u, "r", &meta()).unwrap()).unwrap();
        assert_eq!(second.change_count(), 0);
    }

    #[test]
    fn rendering_is_deterministic() {
        let (d, f, u) = sample();
        assert_eq!(
            render(&d, &f, &u, "r", &meta()).unwrap(),
            render(&d, &f, &u, "r", &meta()).unwrap()
        );
    }

    #[test]
    fn hostile_slugs_cannot_escape_the_docs_dir() {
        assert_eq!(segment("../../etc"), "etc");
        assert_eq!(segment("A b/c"), "a-b-c");
        assert_eq!(segment(".."), "unnamed");
    }

    fn rules() -> Vec<BusinessRule> {
        use retrodoc_core::model::{RuleEvidence, RuleKind};
        vec![
            BusinessRule {
                slug: "pay-invoice-1".to_string(),
                statement: "An invoice already paid\ncannot be paid again.".to_string(),
                kind: RuleKind::StateTransition,
                entities: vec!["Invoice".to_string()],
                use_cases: vec!["pay-by-card".to_string()],
                evidence: vec![
                    RuleEvidence::Code {
                        source: SourceRef {
                            path: "src/pay.rs".to_string(),
                            start_line: Some(3),
                            end_line: Some(9),
                        },
                    },
                    RuleEvidence::Test {
                        phrase: "refuses a second payment".to_string(),
                        path: "spec/pay_spec.rb".to_string(),
                    },
                    RuleEvidence::Doc {
                        path: "README.md".to_string(),
                        heading: "Payments".to_string(),
                    },
                ],
                confidence: Some(ConfidenceScore::new(0.85, None)),
            },
            BusinessRule {
                slug: "pay-invoice-2".to_string(),
                statement: "A payment above the limit is refused.".to_string(),
                kind: RuleKind::Limit,
                entities: vec!["Invoice".to_string(), "Payment".to_string()],
                use_cases: Vec::new(),
                evidence: vec![RuleEvidence::Code {
                    source: SourceRef {
                        path: "src/pay.rs".to_string(),
                        start_line: None,
                        end_line: None,
                    },
                }],
                confidence: Some(ConfidenceScore::new(0.4, None)),
            },
            BusinessRule {
                slug: "list-1".to_string(),
                statement: "Only open invoices are listed.".to_string(),
                kind: RuleKind::Validation,
                entities: Vec::new(),
                use_cases: Vec::new(),
                evidence: Vec::new(),
                confidence: None,
            },
        ]
    }

    #[test]
    fn rules_are_on_the_pages_of_their_feature_use_cases_and_domain() {
        let (d, f, u) = sample();
        let files = render_with_rules(&d, &f, &u, &rules(), "r", &meta()).unwrap();
        let content = |p: &str| {
            files
                .iter()
                .find(|f| f.path.display().to_string() == p)
                .unwrap_or_else(|| panic!("{p} missing"))
                .content
                .clone()
        };

        let feature = content("functional/billing/payment/pay-invoice.md");
        assert!(feature.contains("## Business rules\n\n- An invoice already paid cannot be paid again. (State transition, 85%)"), "{feature}");
        assert!(
            feature.contains("  - Use cases: [Pay by card](use-cases/pay-invoice/pay-by-card.md)"),
            "{feature}"
        );
        assert!(feature.contains("  - Code: `src/pay.rs:3-9`"), "{feature}");
        assert!(
            feature.contains("  - Test: \"refuses a second payment\" (`spec/pay_spec.rb`)"),
            "{feature}"
        );
        assert!(
            feature.contains("  - Documentation: `README.md`, section \"Payments\""),
            "{feature}"
        );
        assert!(feature.contains("(Limit, ⚠️ 40%)"), "{feature}");
        assert!(
            feature.contains("  - Entities: Invoice, Payment"),
            "{feature}"
        );
        assert!(!feature.contains("Only open invoices"), "{feature}");

        let use_case = content("functional/billing/payment/use-cases/pay-invoice/pay-by-card.md");
        assert!(use_case.contains("## Business rules"), "{use_case}");
        assert!(use_case.contains("cannot be paid again"), "{use_case}");
        assert!(!use_case.contains("above the limit"), "{use_case}");

        let domain = content("functional/billing/README.md");
        assert!(domain.contains("## Business rules"), "{domain}");
        assert!(domain.contains("- A payment above the limit is refused. (Limit, ⚠️ 40%) — [Pay an invoice](payment/pay-invoice.md)"), "{domain}");
        assert!(
            domain.contains(
                "Only open invoices are listed. (Validation, not scored) — [List](list.md)"
            ),
            "{domain}"
        );
    }

    #[test]
    fn the_rules_index_groups_by_entity_and_the_agent_index_points_to_it() {
        let (d, f, u) = sample();
        let files = render_with_rules(&d, &f, &u, &rules(), "r", &meta()).unwrap();
        let content = |p: &str| {
            files
                .iter()
                .find(|f| f.path.display().to_string() == p)
                .unwrap()
                .content
                .clone()
        };
        let index = content("functional/rules.md");
        let invoice = index.find("## Invoice").unwrap();
        let payment = index.find("## Payment").unwrap();
        let other = index.find("## Other rules").unwrap();
        assert!(invoice < payment && payment < other, "{index}");
        assert!(index.contains("3 rule(s)"), "{index}");
        assert!(
            index.contains("— [Pay an invoice](billing/payment/pay-invoice.md)"),
            "{index}"
        );
        assert!(
            index.contains(
                "Only open invoices are listed. (Validation, not scored) — [List](billing/list.md)"
            ),
            "{index}"
        );
        assert_eq!(index.matches("A payment above the limit").count(), 2);

        let agents = content("functional/llms.txt");
        assert!(
            agents.contains("[Business rules by entity](rules.md): 3 rule(s)"),
            "{agents}"
        );
    }

    #[test]
    fn a_rule_of_a_vanished_feature_is_neither_counted_nor_indexed() {
        let (d, f, u) = sample();
        let mut all = rules();
        all[2].slug = "gone-1".to_string();
        let files = render_with_rules(&d, &f, &u, &all, "r", &meta()).unwrap();
        let text: String = files.iter().map(|f| f.content.as_str()).collect();
        assert!(!text.contains("Only open invoices"), "{text}");
        assert!(text.contains("2 rule(s)"), "{text}");
    }

    #[test]
    fn without_rules_the_docs_are_those_of_render() {
        let (d, f, u) = sample();
        let plain = render(&d, &f, &u, "r", &meta()).unwrap();
        assert_eq!(
            render_with_rules(&d, &f, &u, &[], "r", &meta()).unwrap(),
            plain
        );
        assert!(plain.iter().all(|f| !f.content.contains("Business rules")));
        assert!(plain.iter().all(|f| !f.path.ends_with("rules.md")));
    }

    #[test]
    fn rules_render_deterministically_and_a_rerun_is_a_no_op() {
        let (d, f, u) = sample();
        let dir = tempfile::tempdir().unwrap();
        let files = || render_with_rules(&d, &f, &u, &rules(), "r", &meta()).unwrap();
        assert_eq!(files(), files());
        crate::plan(dir.path(), files())
            .unwrap()
            .apply(dir.path())
            .unwrap();
        assert_eq!(crate::plan(dir.path(), files()).unwrap().change_count(), 0);
    }
}
