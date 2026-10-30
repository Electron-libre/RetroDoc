//! Documentation debt report (roadmap phase 5): aggregates the confidence
//! scores of the saved artifacts into per-domain figures and a list of the
//! sections that need human attention. Built from `.retrodoc/cache/`, so
//! `retrodoc report` needs no LLM call. [`DebtReport::to_markdown`] is the
//! `coverage-report.md` content the writing phase will put under
//! `docs/_retrodoc/`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use retrodoc_core::model::{ConfidenceScore, Domain, Feature, UseCase, LOW_CONFIDENCE_THRESHOLD};

use crate::behaviour_coverage::{EntryCount, FeatureCoverage};
use crate::domains::{DomainMap, UNCATEGORIZED_SLUG};
use crate::reconcile::Reconciliation;
use crate::rules::SetAside;

mod behaviour;
mod reconciliation;
mod set_aside;

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
    /// Mean business-language score over the scored use cases.
    pub overall_business: Option<f32>,
    /// Least trusted domain first; unscored ones last.
    pub domains: Vec<DomainDebt>,
    /// Use cases and features below the threshold, lowest score first.
    pub weak_sections: Vec<WeakSection>,
    /// Use cases whose business-language score is below the threshold (they
    /// read like code, or like an internal helper), lowest first.
    pub technical_use_cases: Vec<WeakSection>,
    /// Use cases whose narrative the confidence pass judged not backed by the cited code
    /// (`UseCase::narrative_check` below 1), lowest first.
    pub unbacked_narratives: Vec<WeakSection>,
    /// Use cases the docs set apart under "Administration and technical"
    /// (`UseCase::is_technical`), and the features marked back-office.
    pub set_apart_use_cases: usize,
    pub back_office_features: usize,
    /// Use cases the confidence pass could not score.
    pub unscored_use_cases: Vec<String>,
    /// Source files the clustering could not place in a domain: code with no
    /// documentation at all.
    pub uncategorized_files: Vec<PathBuf>,
    /// Source files left out by the file budget (`--max-files`): not
    /// analysed at all. Filled by the caller from the saved `Scope`.
    pub skipped_files: Vec<PathBuf>,
    /// Entry points (`<file>#<name>`) no feature holds. Filled by the caller from the saved
    /// `BehaviourCoverage`.
    pub unplaced_entry_points: Vec<String>,
    /// Per bounded context: its entry points and those held by no feature.
    pub entries_by_context: BTreeMap<String, EntryCount>,
    /// Per feature (from the surface): its entry points and those no use case cites.
    pub entries_by_feature: BTreeMap<String, FeatureCoverage>,
    /// Source files reached by no behaviour: no entry point of a feature, no code run by one, no
    /// entity. Filled by the caller like `unplaced_entry_points`.
    pub technical_support: Vec<PathBuf>,
    /// Where the items stand against the project's own docs. `None` until the caller adds it with
    /// [`DebtReport::with_reconciliation`].
    pub reconciliation: Option<Reconciliation>,
    /// What the rules pass set aside on a judgement of the LLM: to be checked. `None` when nothing.
    pub set_aside: Option<SetAside>,
    /// The slugs of the rules of `set_aside` that `keep_rules` of `retrodoc.toml` puts back.
    pub rules_put_back: BTreeSet<String>,
}

/// Skipped files listed in the Markdown report (the rest is only counted).
const MAX_LISTED_SKIPPED: usize = 50;

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0_f32, 0_u32), |(s, n), v| (s + v, n + 1));
    #[allow(clippy::cast_precision_loss)]
    (n > 0).then(|| sum / n as f32)
}

/// Converts the clustering into the rendering model: the "uncategorized"
/// bucket is left out (it belongs to the coverage report), and each domain
/// carries the mean confidence of its features.
#[must_use]
pub fn domain_models(map: &DomainMap, features: &[Feature]) -> Vec<Domain> {
    map.domains
        .iter()
        .filter(|d| d.slug != UNCATEGORIZED_SLUG)
        .map(|d| Domain {
            slug: d.slug.clone(),
            name: d.name.clone(),
            description: d.description.clone(),
            confidence: mean_confidence(features.iter().filter(|f| f.domain_slug == d.slug)),
        })
        .collect()
}

fn mean_confidence<'a>(features: impl Iterator<Item = &'a Feature>) -> Option<ConfidenceScore> {
    mean(features.filter_map(|f| f.confidence.as_ref().map(|c| c.value)))
        .map(|v| ConfidenceScore::new(v, None))
}

/// Per-domain figures, least trusted domain first (unscored ones last).
fn domain_debts(
    features: &[Feature],
    use_cases: &[UseCase],
    feature_domain: &BTreeMap<&str, &str>,
) -> Vec<DomainDebt> {
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
    domain_debts
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

    let domain_debts = domain_debts(features, use_cases, &feature_domain);

    let back_office: BTreeSet<&str> = features
        .iter()
        .filter(|f| f.back_office)
        .map(|f| f.slug.as_str())
        .collect();
    let set_apart_use_cases = use_cases
        .iter()
        .filter(|u| u.is_technical(back_office.contains(u.feature_slug.as_str())))
        .count();
    let mut weak_sections: Vec<WeakSection> = Vec::new();
    let mut unscored_use_cases = Vec::new();
    let mut technical_use_cases: Vec<WeakSection> = Vec::new();
    let mut unbacked_narratives: Vec<WeakSection> = Vec::new();
    for use_case in use_cases {
        let domain = feature_domain
            .get(use_case.feature_slug.as_str())
            .copied()
            .unwrap_or("?");
        let label = format!("{domain}/{}/{}", use_case.feature_slug, use_case.slug);
        if let Some(b) = &use_case.business_language {
            if b.value < LOW_CONFIDENCE_THRESHOLD {
                technical_use_cases.push(WeakSection {
                    label: label.clone(),
                    score: b.value,
                    rationale: b.rationale.clone(),
                });
            }
        }
        if let Some(n) = use_case.narrative_check.as_ref().filter(|n| n.value < 1.0) {
            unbacked_narratives.push(WeakSection {
                label: label.clone(),
                score: n.value,
                rationale: n.rationale.clone(),
            });
        }
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
    technical_use_cases.sort_by(|a, b| a.score.total_cmp(&b.score).then(a.label.cmp(&b.label)));
    unbacked_narratives.sort_by(|a, b| a.score.total_cmp(&b.score).then(a.label.cmp(&b.label)));

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
        overall_business: mean(
            use_cases
                .iter()
                .filter_map(|u| u.business_language.as_ref().map(|c| c.value)),
        ),
        domains: domain_debts,
        weak_sections,
        technical_use_cases,
        unbacked_narratives,
        set_apart_use_cases,
        back_office_features: back_office.len(),
        unscored_use_cases,
        uncategorized_files,
        skipped_files: Vec::new(),
        unplaced_entry_points: Vec::new(),
        entries_by_context: BTreeMap::new(),
        entries_by_feature: BTreeMap::new(),
        technical_support: Vec::new(),
        reconciliation: None,
        set_aside: None,
        rules_put_back: BTreeSet::new(),
    }
}

/// `text` on one line, so that a sentence of the LLM can't break the list it is written in.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn percent(value: Option<f32>) -> String {
    value.map_or_else(
        || "not scored".to_string(),
        |v| format!("{:.0}%", v * 100.0),
    )
}

impl DebtReport {
    /// The use cases whose narrative the cited code does not back, with the model's reason.
    fn write_narratives_section(&self, md: &mut String) {
        if self.unbacked_narratives.is_empty() {
            return;
        }
        md.push_str("\n## Narratives the cited code does not back\n\n");
        for w in &self.unbacked_narratives {
            let verdict = if w.score > 0.0 {
                "partial"
            } else {
                "unsupported"
            };
            let _ = write!(md, "- `{}` — {verdict}", w.label);
            if let Some(why) = &w.rationale {
                let _ = write!(md, ": {}", one_line(why));
            }
            md.push('\n');
        }
    }

    /// Renders the report as Markdown (`docs/_retrodoc/coverage-report.md`).
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::from("# Documentation coverage report\n\n");
        let _ = writeln!(md, "Overall confidence: **{}**\n", percent(self.overall));
        if self.overall_business.is_some() {
            let _ = writeln!(
                md,
                "Overall business language: **{}**\n",
                percent(self.overall_business)
            );
        }

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

        if self.set_apart_use_cases > 0 || self.back_office_features > 0 {
            let _ = write!(
                md,
                "\n## Set apart in the docs\n\n{} use case(s) and {} back-office feature(s) are listed \
                 under \"Administration and technical\" instead of among the business ones.\n",
                self.set_apart_use_cases, self.back_office_features
            );
        }

        if !self.technical_use_cases.is_empty() {
            let _ = write!(
                md,
                "\n## Technical-sounding use cases (business language below {:.0}%)\n\n",
                LOW_CONFIDENCE_THRESHOLD * 100.0
            );
            for w in &self.technical_use_cases {
                let _ = write!(md, "- `{}` — {}", w.label, percent(Some(w.score)));
                if let Some(why) = &w.rationale {
                    let _ = write!(md, ": {why}");
                }
                md.push('\n');
            }
        }

        self.write_narratives_section(&mut md);

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
        self.write_behaviour_sections(&mut md);
        self.write_reconciliation_section(&mut md);
        self.write_set_aside_section(&mut md);
        if !self.skipped_files.is_empty() {
            let _ = write!(
                md,
                "\n## Not analysed (file budget)\n\n{} source file(s) were left out, the least \
                 important by role, history and references (full list in `.retrodoc/cache/scope.yaml`):\n\n",
                self.skipped_files.len()
            );
            for path in self.skipped_files.iter().take(MAX_LISTED_SKIPPED) {
                let _ = writeln!(md, "- `{}`", path.display());
            }
            if self.skipped_files.len() > MAX_LISTED_SKIPPED {
                let _ = writeln!(
                    md,
                    "- … and {} more",
                    self.skipped_files.len() - MAX_LISTED_SKIPPED
                );
            }
        }
        md
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod set_apart_tests;
