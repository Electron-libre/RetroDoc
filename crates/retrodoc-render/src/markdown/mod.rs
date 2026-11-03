//! Markdown/Mermaid rendering of the domain model (docs/ARCHITECTURE.md §7 layout):
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
    BusinessRule, ConfidenceScore, Domain, Feature, SourceRef, UseCase, LOW_CONFIDENCE_THRESHOLD,
};
use retrodoc_core::text::one_line;

use crate::doc_links::DocLinks;
use crate::rules::index_page;
use crate::texts::Texts;
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

/// Renders every file of the docs in English, paths relative to the docs dir. Pure and
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
        &Texts::ENGLISH,
        coverage_report,
        metadata,
    )
}

/// [`render_with_rules`] with the sections of the project's own docs that document or contradict
/// each item, linked from its page (a rule: under it), the fixed text from `texts`. Without links,
/// and in English, the files are the same.
///
/// # Errors
///
/// Returns an error if the run metadata can't be serialized.
#[allow(clippy::too_many_arguments)] // the artifacts of the run, then how and what to write around them
pub fn render_with_docs(
    domains: &[Domain],
    features: &[Feature],
    use_cases: &[UseCase],
    rules: &[BusinessRule],
    docs: &DocLinks,
    texts: &Texts,
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
            content: domain_page(domain, &domain_features, rules, docs, texts),
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
                content: feature_page(feature, &own, &own_rules, docs, texts),
            });
            for use_case in &own {
                let applying: Vec<&BusinessRule> = own_rules
                    .iter()
                    .copied()
                    .filter(|r| r.use_cases.contains(&use_case.slug))
                    .collect();
                files.push(RenderedFile {
                    path: use_case_path(feature, use_case),
                    content: use_case_page(feature, use_case, &applying, docs, texts),
                });
            }
        }
    }

    if !rules.is_empty() {
        files.push(RenderedFile {
            path: PathBuf::from(RULES_INDEX_PATH),
            content: index_page(rules, features, Path::new(RULES_INDEX_PATH), texts),
        });
    }
    files.push(RenderedFile {
        path: PathBuf::from(AGENT_INDEX_PATH),
        content: agent_index(domains, features, rules.len(), texts),
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

/// Cited lines as the docs show them: `path:start-end`, `path:start`, or the path alone.
#[must_use]
pub fn source_ref_label(source: &SourceRef) -> String {
    match (source.start_line, source.end_line) {
        (Some(s), Some(e)) if s != e => format!("{}:{s}-{e}", source.path),
        (Some(s), _) => format!("{}:{s}", source.path),
        _ => source.path.clone(),
    }
}

fn confidence_line(score: Option<&ConfidenceScore>, texts: &Texts) -> String {
    score_line(texts.confidence, score, texts)
}

/// `**<label>** ⚠️ 40% — why`, the label with its colon, the flag when below the threshold.
fn score_line(label: &str, score: Option<&ConfidenceScore>, texts: &Texts) -> String {
    let Some(score) = score else {
        return format!("**{label}** {}", texts.not_scored);
    };
    let mut line = format!(
        "**{label}** {}{:.0}%",
        low_confidence_flag(Some(score)),
        score.value * 100.0
    );
    if let Some(why) = score.rationale.as_deref().filter(|r| !r.trim().is_empty()) {
        let _ = write!(line, " — {}", one_line(why));
    }
    line
}

/// `- [name](link) (score): description` on one line, `indent` before it: the item of a list of
/// the pages and of the agent index, `link` already relative to the page.
fn list_item(
    md: &mut String,
    indent: &str,
    name: &str,
    link: &str,
    score: Option<&ConfidenceScore>,
    description: &str,
    texts: &Texts,
) {
    let _ = writeln!(
        md,
        "{indent}- [{}]({link}) ({}{}): {}",
        one_line(name),
        low_confidence_flag(score),
        short_confidence(score, texts),
        one_line(description)
    );
}

/// `⚠️ ` below the threshold, so a weak item is visible before opening it.
fn low_confidence_flag(score: Option<&ConfidenceScore>) -> &'static str {
    match score {
        Some(c) if c.value < LOW_CONFIDENCE_THRESHOLD => "⚠️ ",
        _ => "",
    }
}

fn short_confidence(score: Option<&ConfidenceScore>, texts: &Texts) -> String {
    score.map_or_else(
        || texts.not_scored.to_string(),
        |c| format!("{:.0}%", c.value * 100.0),
    )
}

/// Exactly one trailing newline.
fn trim_end(md: &str) -> String {
    format!("{}\n", md.trim_end())
}

#[cfg(test)]
mod tests;
