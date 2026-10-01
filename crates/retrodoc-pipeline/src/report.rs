//! Documentation debt report (roadmap phase 5): aggregates the confidence
//! scores of the saved artifacts into per-domain figures and a list of the
//! sections that need human attention. Built from `.retrodoc/cache/`, so
//! `retrodoc report` needs no LLM call. [`DebtReport::to_markdown`] is the
//! `coverage-report.md` content the writing phase will put under
//! `docs/_retrodoc/`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use retrodoc_core::model::{Feature, UseCase};

use crate::domains::{DomainMap, UNCATEGORIZED_SLUG};

/// Sections scoring below this are listed as documentation debt.
pub const LOW_CONFIDENCE_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub struct DomainDebt {
    pub slug: String,
    pub features: usize,
    pub use_cases: usize,
    /// Mean feature confidence; `None` if none is scored.
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeakSection {
    /// e.g. `billing/payment/pay-invoice`.
    pub label: String,
    pub score: f32,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DebtReport {
    /// Mean confidence over all scored use cases.
    pub overall: Option<f32>,
    /// Least trusted domain first; unscored ones last.
    pub domains: Vec<DomainDebt>,
    /// Use cases and features below the threshold, lowest score first.
    pub weak_sections: Vec<WeakSection>,
    /// Use cases the confidence pass could not score.
    pub unscored_use_cases: Vec<String>,
    /// Source files the clustering could not place in a domain: code with no
    /// documentation at all.
    pub uncategorized_files: Vec<PathBuf>,
}

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0_f32, 0_u32), |(s, n), v| (s + v, n + 1));
    #[allow(clippy::cast_precision_loss)]
    (n > 0).then(|| sum / n as f32)
}

/// Builds the report from the pipeline artifacts.
#[must_use]
pub fn build_report(
    domains: &DomainMap,
    features: &[Feature],
    use_cases: &[UseCase],
) -> DebtReport {
    let feature_domain: BTreeMap<&str, &str> = features
        .iter()
        .map(|f| (f.slug.as_str(), f.domain_slug.as_str()))
        .collect();

    let mut per_domain: BTreeMap<&str, DomainDebt> = BTreeMap::new();
    for feature in features {
        let entry = per_domain
            .entry(feature.domain_slug.as_str())
            .or_insert_with(|| DomainDebt {
                slug: feature.domain_slug.clone(),
                features: 0,
                use_cases: 0,
                confidence: None,
            });
        entry.features += 1;
    }
    for use_case in use_cases {
        if let Some(entry) = feature_domain
            .get(use_case.feature_slug.as_str())
            .and_then(|d| per_domain.get_mut(d))
        {
            entry.use_cases += 1;
        }
    }
    for (slug, entry) in &mut per_domain {
        entry.confidence = mean(
            features
                .iter()
                .filter(|f| f.domain_slug == *slug)
                .filter_map(|f| f.confidence.as_ref().map(|c| c.value)),
        );
    }
    let mut domain_debts: Vec<DomainDebt> = per_domain.into_values().collect();
    domain_debts.sort_by(|a, b| match (a.confidence, b.confidence) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.slug.cmp(&b.slug),
    });

    let mut weak_sections: Vec<WeakSection> = Vec::new();
    let mut unscored_use_cases = Vec::new();
    for use_case in use_cases {
        let domain = feature_domain
            .get(use_case.feature_slug.as_str())
            .copied()
            .unwrap_or("?");
        let label = format!("{domain}/{}/{}", use_case.feature_slug, use_case.slug);
        match &use_case.confidence {
            Some(c) if c.value < LOW_CONFIDENCE_THRESHOLD => weak_sections.push(WeakSection {
                label,
                score: c.value,
                rationale: c.rationale.clone(),
            }),
            Some(_) => {}
            None => unscored_use_cases.push(label),
        }
    }
    for feature in features {
        if let Some(c) = &feature.confidence {
            // A feature only gets its own line when no use case explains its
            // low score (i.e. it has none).
            if c.value < LOW_CONFIDENCE_THRESHOLD
                && !use_cases.iter().any(|u| u.feature_slug == feature.slug)
            {
                weak_sections.push(WeakSection {
                    label: format!("{}/{}", feature.domain_slug, feature.slug),
                    score: c.value,
                    rationale: c.rationale.clone(),
                });
            }
        }
    }
    weak_sections.sort_by(|a, b| a.score.total_cmp(&b.score).then(a.label.cmp(&b.label)));

    let uncategorized_files = domains
        .domains
        .iter()
        .filter(|d| d.slug == UNCATEGORIZED_SLUG)
        .flat_map(|d| d.paths.iter().cloned())
        .collect();

    DebtReport {
        overall: mean(
            use_cases
                .iter()
                .filter_map(|u| u.confidence.as_ref().map(|c| c.value)),
        ),
        domains: domain_debts,
        weak_sections,
        unscored_use_cases,
        uncategorized_files,
    }
}

fn percent(value: Option<f32>) -> String {
    value.map_or_else(
        || "not scored".to_string(),
        |v| format!("{:.0}%", v * 100.0),
    )
}

impl DebtReport {
    /// Renders the report as Markdown (`docs/_retrodoc/coverage-report.md`).
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::from("# Documentation coverage report\n\n");
        let _ = writeln!(md, "Overall confidence: **{}**\n", percent(self.overall));

        md.push_str(
            "## Domains\n\n| Domain | Features | Use cases | Confidence |\n|---|---|---|---|\n",
        );
        for d in &self.domains {
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} |",
                d.slug,
                d.features,
                d.use_cases,
                percent(d.confidence)
            );
        }

        let _ = write!(
            md,
            "\n## Low-confidence sections (below {:.0}%)\n\n",
            LOW_CONFIDENCE_THRESHOLD * 100.0
        );
        if self.weak_sections.is_empty() {
            md.push_str("None.\n");
        }
        for w in &self.weak_sections {
            let _ = write!(md, "- `{}` — {}", w.label, percent(Some(w.score)));
            if let Some(why) = &w.rationale {
                let _ = write!(md, ": {why}");
            }
            md.push('\n');
        }

        if !self.unscored_use_cases.is_empty() {
            md.push_str("\n## Not scored\n\n");
            for label in &self.unscored_use_cases {
                let _ = writeln!(md, "- `{label}`");
            }
        }

        if !self.uncategorized_files.is_empty() {
            md.push_str("\n## Undocumented code (no domain found)\n\n");
            for path in &self.uncategorized_files {
                let _ = writeln!(md, "- `{}`", path.display());
            }
        }
        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use retrodoc_core::model::ConfidenceScore;

    use crate::domains::DomainCluster;

    fn feature(slug: &str, domain: &str, score: Option<f32>) -> Feature {
        Feature {
            slug: slug.to_string(),
            domain_slug: domain.to_string(),
            sub_domain_slug: None,
            name: slug.to_string(),
            description: String::new(),
            source_paths: vec![],
            confidence: score.map(|s| ConfidenceScore::new(s, None)),
        }
    }

    fn use_case(slug: &str, feature: &str, score: Option<f32>) -> UseCase {
        UseCase {
            slug: slug.to_string(),
            feature_slug: feature.to_string(),
            name: slug.to_string(),
            description: String::new(),
            steps: vec![],
            diagram_mermaid: None,
            confidence: score.map(|s| ConfidenceScore::new(s, "because".to_string())),
        }
    }

    #[test]
    fn aggregates_per_domain_and_lists_debt() {
        let domains = DomainMap {
            domains: vec![DomainCluster {
                slug: UNCATEGORIZED_SLUG.to_string(),
                name: String::new(),
                description: String::new(),
                paths: vec![PathBuf::from("x.rs")],
                sub_domains: vec![],
            }],
        };
        let features = vec![
            feature("pay", "billing", Some(0.2)),
            feature("login", "auth", Some(0.9)),
            feature("orphan", "auth", Some(0.0)),
        ];
        let use_cases = vec![
            use_case("pay-invoice", "pay", Some(0.2)),
            use_case("sign-in", "login", Some(0.9)),
            use_case("mystery", "login", None),
        ];

        let report = build_report(&domains, &features, &use_cases);

        assert!((report.overall.unwrap() - 0.55).abs() < 1e-6);
        // Least trusted domain first: billing 0.2, auth (0.9 + 0.0) / 2.
        assert_eq!(report.domains[0].slug, "billing");
        assert_eq!(report.domains[1].slug, "auth");
        assert!((report.domains[1].confidence.unwrap() - 0.45).abs() < 1e-6);
        assert_eq!(
            (report.domains[1].features, report.domains[1].use_cases),
            (2, 2)
        );
        // Lowest first; "orphan" has no use case so it is listed itself.
        let labels: Vec<&str> = report
            .weak_sections
            .iter()
            .map(|w| w.label.as_str())
            .collect();
        assert_eq!(labels, ["auth/orphan", "billing/pay/pay-invoice"]);
        assert_eq!(report.unscored_use_cases, ["auth/login/mystery"]);
        assert_eq!(report.uncategorized_files, [PathBuf::from("x.rs")]);

        let md = report.to_markdown();
        assert!(md.contains("Overall confidence: **55%**"));
        assert!(md.contains("`billing/pay/pay-invoice` — 20%: because"));
        assert!(md.contains("`x.rs`"));
    }
}
