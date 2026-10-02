//! Markdown/Mermaid rendering of the domain model (PLAN.md §3 layout):
//!
//! ```text
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
    ConfidenceScore, Domain, Feature, Step, UseCase, LOW_CONFIDENCE_THRESHOLD,
};

use crate::{RenderedFile, RunMetadata, COVERAGE_REPORT_PATH, FUNCTIONAL_DIR, METADATA_PATH};

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
    let mut files = Vec::new();

    for domain in domains {
        let domain_features: Vec<&Feature> = features
            .iter()
            .filter(|f| f.domain_slug == domain.slug)
            .collect();
        files.push(RenderedFile {
            path: domain_readme_path(&domain.slug),
            content: domain_page(domain, &domain_features),
        });
        for feature in domain_features {
            let own: Vec<&UseCase> = use_cases
                .iter()
                .filter(|u| u.feature_slug == feature.slug)
                .collect();
            files.push(RenderedFile {
                path: feature_path(feature),
                content: feature_page(feature, &own),
            });
            for use_case in own {
                files.push(RenderedFile {
                    path: use_case_path(feature, use_case),
                    content: use_case_page(feature, use_case),
                });
            }
        }
    }

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

fn feature_path(feature: &Feature) -> PathBuf {
    container(feature).join(format!("{}.md", segment(&feature.slug)))
}

fn use_case_path(feature: &Feature, use_case: &UseCase) -> PathBuf {
    container(feature)
        .join("use-cases")
        .join(segment(&feature.slug))
        .join(format!("{}.md", segment(&use_case.slug)))
}

/// Relative Markdown link target from the file `from` to the file `to`.
fn link(from: &Path, to: &Path) -> String {
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
    let Some(score) = score else {
        return "**Confidence:** not scored".to_string();
    };
    let flag = if score.value < LOW_CONFIDENCE_THRESHOLD {
        "⚠️ "
    } else {
        ""
    };
    let mut line = format!("**Confidence:** {flag}{:.0}%", score.value * 100.0);
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

fn domain_page(domain: &Domain, features: &[&Feature]) -> String {
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
    trim_end(&md)
}

fn feature_page(feature: &Feature, use_cases: &[&UseCase]) -> String {
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

    if !feature.source_paths.is_empty() {
        md.push_str("\n## Source files\n\n");
        for p in &feature.source_paths {
            let _ = writeln!(md, "- `{p}`");
        }
    }
    trim_end(&md)
}

fn use_case_page(feature: &Feature, use_case: &UseCase) -> String {
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

    md.push_str("## Steps\n\n");
    for step in &use_case.steps {
        md.push_str(&step_item(step));
    }

    if let Some(diagram) = &use_case.diagram_mermaid {
        let _ = write!(
            md,
            "\n## Diagram\n\n```mermaid\n{}\n```\n",
            diagram.trim_end()
        );
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
                confidence: Some(ConfidenceScore::new(0.3, "weak".to_string())),
            },
            Feature {
                slug: "list".to_string(),
                domain_slug: "billing".to_string(),
                sub_domain_slug: None,
                name: "List".to_string(),
                description: "Listing.".to_string(),
                source_paths: vec![],
                confidence: None,
            },
        ];
        let use_cases = vec![UseCase {
            entry_points: Vec::new(),
            primary_actor: None,
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
}
