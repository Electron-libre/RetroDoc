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
    BusinessRule, ConfidenceScore, Domain, Feature, UseCase, LOW_CONFIDENCE_THRESHOLD,
};

use crate::doc_links::DocLinks;
use crate::rules::index_page;
use crate::{
    RenderedFile, RunMetadata, AGENT_INDEX_PATH, COVERAGE_REPORT_PATH, FUNCTIONAL_DIR,
    METADATA_PATH, RULES_INDEX_PATH,
};

mod agent_index;
mod domain;
mod feature;
mod use_case;

use agent_index::agent_index;
use domain::domain_page;
use feature::feature_page;
use use_case::use_case_page;

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

/// Exactly one trailing newline.
fn trim_end(md: &str) -> String {
    format!("{}\n", md.trim_end())
}

#[cfg(test)]
mod tests;
