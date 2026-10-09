//! What `generate` prints about each pass once it is done.

use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_pipeline::{Artifact, CoverageReport, DomainMap, RepoMap};

pub(super) fn repo_map(map: &RepoMap) {
    println!("\nModules:");
    for module in &map.modules {
        let label = if module.path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            module.path.display().to_string()
        };
        println!(
            "  {label} ({} file(s)): {}",
            module.file_count, module.role_summary
        );
    }

    println!("\nFiles:");
    for file in &map.files {
        println!("  {}: {}", file.path.display(), file.role_summary);
    }
}

pub(super) fn domain_map(map: &DomainMap) {
    println!("\nDomains:");
    for domain in &map.domains {
        println!(
            "  {} ({}): {} file(s) directly, {} sub-domain(s)",
            domain.name,
            domain.slug,
            domain.paths.len(),
            domain.sub_domains.len()
        );
        for sub in &domain.sub_domains {
            println!(
                "    {} ({}): {} file(s)",
                sub.name,
                sub.slug,
                sub.paths.len()
            );
        }
    }
}

pub(super) fn features(features: &[Feature], use_cases: &[UseCase]) {
    println!("\nFeatures:");
    for feature in features {
        println!(
            "  {}/{} — {} [{}]",
            feature.domain_slug,
            feature.slug,
            feature.name,
            confidence_label(feature.confidence.as_ref())
        );
        for use_case in use_cases.iter().filter(|u| u.feature_slug == feature.slug) {
            println!(
                "    - {} ({} step(s)) [{}]",
                use_case.name,
                use_case.steps.len(),
                confidence_label(use_case.confidence.as_ref())
            );
        }
    }
}

fn confidence_label(score: Option<&ConfidenceScore>) -> String {
    score.map_or_else(
        || "not scored".to_string(),
        |c| format!("{:.0}%", c.value * 100.0),
    )
}

pub(super) fn coverage_report(report: &CoverageReport) {
    if report.is_clean() {
        println!("\nCoverage: 100% of source files assigned, no overlap.");
        return;
    }
    println!("\nCoverage issues found (repaired automatically):");
    if !report.uncovered.is_empty() {
        println!(
            "  {} file(s) unassigned by the LLM, bucketed into \"uncategorized\".",
            report.uncovered.len()
        );
    }
    if !report.overlapping.is_empty() {
        println!(
            "  {} file(s) assigned to more than one domain, kept only the first.",
            report.overlapping.len()
        );
    }
    if !report.unknown.is_empty() {
        println!(
            "  {} unknown path(s) cited by the LLM, dropped.",
            report.unknown.len()
        );
    }
}

pub(super) fn saved(features: usize, use_cases: usize) {
    println!(
        "\n{features} feature(s) saved to {}, {use_cases} use case(s) to {}.",
        Artifact::Features.relative_path(),
        Artifact::UseCases.relative_path()
    );
    println!("Run `retrodoc report` for the documentation debt report.");
}
