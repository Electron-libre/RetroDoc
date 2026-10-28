//! What `generate` prints about each pass once it is done.

use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_pipeline::{Artifact, BehaviourCoverage, DomainMap};

/// The domains built from the surface: what each holds.
pub(super) fn surface_domains(map: &DomainMap) {
    println!("\nDomains:");
    for domain in &map.domains {
        println!(
            "  {} ({}): {} resource(s), {} entit(ies), {} file(s)",
            domain.name,
            domain.slug,
            domain.resources.len(),
            domain.entities.len(),
            domain.paths.len()
        );
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

pub(super) fn saved(features: usize, use_cases: usize) {
    println!(
        "\n{features} feature(s) saved to {}, {use_cases} use case(s) to {}.",
        Artifact::Features.relative_path(),
        Artifact::UseCases.relative_path()
    );
    println!("Run `retrodoc report` for the documentation debt report.");
}

pub(super) fn behaviour_coverage(coverage: &BehaviourCoverage, source_files: usize) {
    println!(
        "\nBehaviour coverage: {} entry point(s) without a feature, {} of {} source file(s) reached \
         by no behaviour (technical support, listed in the coverage report).",
        coverage.unplaced_entry_points.len(),
        coverage.technical_support.len(),
        source_files
    );
}

/// How many of the entry points held by `features` (those of this run) no use case cites; nothing
/// for features built from files.
pub(super) fn use_case_coverage(coverage: &BehaviourCoverage, features: &[Feature]) {
    let counts: Vec<_> = features
        .iter()
        .filter_map(|f| coverage.by_feature.get(&f.slug))
        .collect();
    let total: usize = counts.iter().map(|c| c.entry_points).sum();
    let uncovered: usize = counts.iter().map(|c| c.uncovered.len()).sum();
    if total > 0 {
        println!(
            "Use cases cover {} of {total} entry point(s) held by features; {uncovered} cited by none.",
            total - uncovered
        );
    }
}

/// The entry points held by no feature, for each context of the run (nothing when none has a count).
pub(super) fn entries_by_context(coverage: &BehaviourCoverage, in_run: &DomainMap) {
    let counts: Vec<_> = in_run
        .domains
        .iter()
        .filter_map(|d| {
            coverage
                .by_domain
                .get(&d.slug)
                .map(|count| (&d.slug, count))
        })
        .collect();
    if counts.is_empty() {
        return;
    }
    println!("\nEntry points by context:");
    for (slug, count) in counts {
        println!(
            "  {slug}: {} entry point(s), {} held by no feature.",
            count.entry_points, count.unplaced
        );
    }
}
