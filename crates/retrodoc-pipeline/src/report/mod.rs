//! Documentation debt report (roadmap phase 5): aggregates the confidence
//! scores of the saved artifacts into per-domain figures and a list of the
//! sections that need human attention. Built from `.retrodoc/cache/`, so
//! `retrodoc report` needs no LLM call. [`DebtReport::to_markdown`] is the
//! `coverage-report.md` content the writing phase will put under
//! `docs/_retrodoc/`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use retrodoc_core::model::{
    ConfidenceScore, Domain, Feature, SetApartReason, UseCase, LOW_CONFIDENCE_THRESHOLD,
};

use crate::behaviour_coverage::{EntryCount, FeatureCoverage};
use crate::domains::DomainMap;
use crate::reconcile::{ItemKind, Reconciliation};
use crate::rules::SetAside;
use crate::text::one_line;

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

/// A feature set aside as technical, as the debt report lists it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TechnicalFeature {
    pub domain_slug: String,
    pub name: String,
    pub entry_points: usize,
}

/// A domain no feature belongs to, left out of the docs, as the debt report lists it (EDM-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeaturelessDomain {
    pub slug: String,
    pub name: String,
    /// Its features set aside as technical; filled with [`DebtReport::with_technical_features`].
    pub set_aside: usize,
}

/// The section of the docs an item of the report points to: a use case, or a feature. Ordered as
/// its [`Section::label`], a domain not known first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Section {
    /// `None` when the feature is not in the run (artifacts of an older run).
    pub domain: Option<String>,
    pub feature: String,
    pub use_case: Option<String>,
}

impl Section {
    fn of_use_case(use_case: &UseCase, feature_domain: &BTreeMap<&str, &str>) -> Self {
        Self {
            domain: feature_domain
                .get(use_case.feature_slug.as_str())
                .map(|d| (*d).to_string()),
            feature: use_case.feature_slug.clone(),
            use_case: Some(use_case.slug.clone()),
        }
    }

    /// `domain/feature/use-case`, or `domain/feature`; `?` for a domain not known.
    #[must_use]
    pub fn label(&self) -> String {
        let domain = self.domain.as_deref().unwrap_or("?");
        match &self.use_case {
            Some(use_case) => format!("{domain}/{}/{use_case}", self.feature),
            None => format!("{domain}/{}", self.feature),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeakSection {
    pub section: Section,
    pub score: f32,
    pub rationale: Option<String>,
}

/// A use case the docs set apart under "Administration and technical", and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetApartUseCase {
    pub section: Section,
    pub reasons: Vec<SetApartReason>,
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
    /// Use cases whose narrative the confidence pass judged contradicted by the cited code
    /// (`UseCase::narrative_check` at 0, `unsupported`). A `partial` one is not listed: it states
    /// what the cited lines do not show, which is mostly true of the code around them.
    pub unbacked_narratives: Vec<WeakSection>,
    /// Use cases the docs set apart under "Administration and technical"
    /// (`UseCase::is_technical`), in the order of the run, and the features marked back-office.
    pub set_apart_use_cases: Vec<SetApartUseCase>,
    pub back_office_features: usize,
    /// Use cases the confidence pass could not score.
    pub unscored_use_cases: Vec<Section>,
    /// Source files the clustering could not place in a domain: code with no
    /// documentation at all.
    pub uncategorized_files: Vec<PathBuf>,
    /// Domains no feature belongs to, left out of the docs (EDM-5).
    pub featureless_domains: Vec<FeaturelessDomain>,
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
    /// The features the features pass set aside as technical, out of the docs. Filled by the caller
    /// with [`DebtReport::with_technical_features`].
    pub technical_features: Vec<TechnicalFeature>,
    /// Where the items stand against the project's own docs. `None` until the caller adds it with
    /// [`DebtReport::with_reconciliation`].
    pub reconciliation: Option<Reconciliation>,
    /// What the rules pass set aside on a judgement of the LLM: to be checked. `None` when nothing.
    pub set_aside: Option<SetAside>,
    /// The slugs of the rules of `set_aside` that `keep_rules` of `retrodoc.toml` puts back.
    pub rules_put_back: BTreeSet<String>,
    /// The domains, features and use cases of the run a human marked wrong, as the reviews are
    /// saved (`specs/debt-view.md` DBT-35).
    pub marked_wrong: Vec<MarkedWrong>,
}

/// A section a human marked wrong, and the comment of the review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkedWrong {
    pub kind: ItemKind,
    /// `<domain>`, `<domain>/<feature>` or `<domain>/<feature>/<use case>`.
    pub label: String,
    /// Empty when the review has none.
    pub comment: String,
    /// Why the review no longer vouches for the section as it is now (DBT-38).
    pub outdated: Option<OutdatedReview>,
}

/// Why a review no longer vouches for its section: rewritten since, or its cited files changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutdatedReview {
    pub rewritten: bool,
    /// The cited files modified or deleted since the review, in path order.
    pub changed: Vec<ChangedFile>,
}

/// A file cited by a reviewed section that changed since the review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    /// Deleted, else modified.
    pub deleted: bool,
}

impl OutdatedReview {
    /// The reasons that apply, as `retrodoc report` prints them after "outdated:": the section
    /// rewritten since the review, the files changed since the review.
    #[must_use]
    pub fn reasons(&self) -> String {
        let mut reasons = Vec::new();
        if self.rewritten {
            reasons.push("section rewritten since the review".to_string());
        }
        if !self.changed.is_empty() {
            let files: Vec<String> = self
                .changed
                .iter()
                .map(|f| {
                    let state = if f.deleted { "deleted" } else { "modified" };
                    format!("`{}` ({state})", one_line(&f.path))
                })
                .collect();
            reasons.push(format!(
                "files changed since the review: {}",
                files.join(", ")
            ));
        }
        reasons.join("; ")
    }
}

/// Skipped files listed in the Markdown report (the rest is only counted).
const MAX_LISTED_SKIPPED: usize = 50;

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0_f32, 0_u32), |(s, n), v| (s + v, n + 1));
    #[allow(clippy::cast_precision_loss)]
    (n > 0).then(|| sum / n as f32)
}

/// Converts the clustering into the rendering model: the "uncategorized"
/// bucket and the domains with no feature are left out (they belong to the
/// coverage report, EDM), and each domain carries the mean confidence of its features.
#[must_use]
pub fn domain_models(map: &DomainMap, features: &[Feature]) -> Vec<Domain> {
    map.published(features)
        .map(|d| Domain {
            slug: d.slug.clone(),
            name: d.name.clone(),
            description: d.description.clone(),
            confidence: mean_confidence(features.iter().filter(|f| f.domain_slug == d.slug)),
        })
        .collect()
}

/// The domains no feature belongs to, their features set aside counted later (EDM-5).
fn featureless_domains(map: &DomainMap, features: &[Feature]) -> Vec<FeaturelessDomain> {
    map.featureless(features)
        .map(|d| FeaturelessDomain {
            slug: d.slug.clone(),
            name: d.name.clone(),
            set_aside: 0,
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

/// The use cases the docs set apart, in the order of the run, with their reasons.
fn set_apart(
    use_cases: &[UseCase],
    back_office: &BTreeSet<&str>,
    feature_domain: &BTreeMap<&str, &str>,
) -> Vec<SetApartUseCase> {
    use_cases
        .iter()
        .filter_map(|u| {
            let reasons = u.set_apart_reasons(back_office.contains(u.feature_slug.as_str()));
            (!reasons.is_empty()).then(|| SetApartUseCase {
                section: Section::of_use_case(u, feature_domain),
                reasons,
            })
        })
        .collect()
}

/// The features below the threshold that have no use case: only then does a feature get its own
/// line, no use case explaining its low score.
fn weak_features<'a>(
    features: &'a [Feature],
    use_cases: &'a [UseCase],
) -> impl Iterator<Item = WeakSection> + 'a {
    features.iter().filter_map(|feature| {
        let c = feature.confidence.as_ref()?;
        (c.value < LOW_CONFIDENCE_THRESHOLD
            && !use_cases.iter().any(|u| u.feature_slug == feature.slug))
        .then(|| WeakSection {
            section: Section {
                domain: Some(feature.domain_slug.clone()),
                feature: feature.slug.clone(),
                use_case: None,
            },
            score: c.value,
            rationale: c.rationale.clone(),
        })
    })
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
    let set_apart_use_cases = set_apart(use_cases, &back_office, &feature_domain);
    let mut weak_sections: Vec<WeakSection> = Vec::new();
    let mut unscored_use_cases = Vec::new();
    let mut technical_use_cases: Vec<WeakSection> = Vec::new();
    let mut unbacked_narratives: Vec<WeakSection> = Vec::new();
    for use_case in use_cases {
        let section = Section::of_use_case(use_case, &feature_domain);
        if let Some(b) = &use_case.business_language {
            if b.value < LOW_CONFIDENCE_THRESHOLD {
                technical_use_cases.push(WeakSection {
                    section: section.clone(),
                    score: b.value,
                    rationale: b.rationale.clone(),
                });
            }
        }
        if let Some(n) = use_case.narrative_check.as_ref().filter(|n| n.value <= 0.0) {
            unbacked_narratives.push(WeakSection {
                section: section.clone(),
                score: n.value,
                rationale: n.rationale.clone(),
            });
        }
        match &use_case.confidence {
            Some(c) if c.value < LOW_CONFIDENCE_THRESHOLD => weak_sections.push(WeakSection {
                section,
                score: c.value,
                rationale: c.rationale.clone(),
            }),
            Some(_) => {}
            None => unscored_use_cases.push(section),
        }
    }
    weak_sections.extend(weak_features(features, use_cases));
    let by_score = |a: &WeakSection, b: &WeakSection| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| a.section.cmp(&b.section))
    };
    weak_sections.sort_by(by_score);
    technical_use_cases.sort_by(by_score);
    unbacked_narratives.sort_by(|a, b| a.section.cmp(&b.section));

    let uncategorized_files = domains.uncategorized_paths().cloned().collect();

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
        featureless_domains: featureless_domains(domains, features),
        // Filled by the caller from the saved artifacts.
        ..DebtReport::default()
    }
}

fn percent(value: Option<f32>) -> String {
    value.map_or_else(
        || "not scored".to_string(),
        |v| format!("{:.0}%", v * 100.0),
    )
}

impl DebtReport {
    /// The domains no feature belongs to, left out of the docs (EDM-5).
    fn write_featureless_domains(&self, md: &mut String) {
        if self.featureless_domains.is_empty() {
            return;
        }
        md.push_str(
            "\n## Domains with no feature\n\nNo feature belongs to these domains (named for \
             entities alone, or with every feature set aside as technical); they are absent from \
             the functional docs:\n\n| Domain | Features set aside |\n|---|---|\n",
        );
        for domain in &self.featureless_domains {
            let _ = writeln!(md, "| {} | {} |", domain.name, domain.set_aside);
        }
    }

    /// The use cases whose narrative the cited code contradicts, with the model's reason.
    fn write_narratives_section(&self, md: &mut String) {
        if self.unbacked_narratives.is_empty() {
            return;
        }
        md.push_str("\n## Narratives the cited code contradicts\n\n");
        for w in &self.unbacked_narratives {
            let _ = write!(md, "- `{}` — unsupported", w.section.label());
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
            let _ = write!(md, "- `{}` — {}", w.section.label(), percent(Some(w.score)));
            if let Some(why) = &w.rationale {
                let _ = write!(md, ": {why}");
            }
            md.push('\n');
        }

        self.write_marked_wrong(&mut md);

        if !self.set_apart_use_cases.is_empty() || self.back_office_features > 0 {
            let _ = write!(
                md,
                "\n## Set apart in the docs\n\n{} use case(s) and {} back-office feature(s) are listed \
                 under \"Administration and technical\" instead of among the business ones.\n",
                self.set_apart_use_cases.len(),
                self.back_office_features
            );
        }

        if !self.technical_use_cases.is_empty() {
            let _ = write!(
                md,
                "\n## Technical-sounding use cases (business language below {:.0}%)\n\n",
                LOW_CONFIDENCE_THRESHOLD * 100.0
            );
            for w in &self.technical_use_cases {
                let _ = write!(md, "- `{}` — {}", w.section.label(), percent(Some(w.score)));
                if let Some(why) = &w.rationale {
                    let _ = write!(md, ": {why}");
                }
                md.push('\n');
            }
        }

        self.write_narratives_section(&mut md);

        if !self.unscored_use_cases.is_empty() {
            md.push_str("\n## Not scored\n\n");
            for section in &self.unscored_use_cases {
                let _ = writeln!(md, "- `{}`", section.label());
            }
        }

        if !self.uncategorized_files.is_empty() {
            md.push_str("\n## Undocumented code (no domain found)\n\n");
            for path in &self.uncategorized_files {
                let _ = writeln!(md, "- `{}`", path.display());
            }
        }
        self.write_featureless_domains(&mut md);
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

impl DebtReport {
    /// The sections a human marked wrong, under their heading when there is one.
    fn write_marked_wrong(&self, md: &mut String) {
        if self.marked_wrong.is_empty() {
            return;
        }
        md.push_str("\n## Sections marked wrong\n\n");
        for section in &self.marked_wrong {
            let _ = write!(md, "- {} `{}`", section.kind.label(), section.label);
            let comment = one_line(&section.comment);
            if !comment.is_empty() {
                let _ = write!(md, ": {comment}");
            }
            if let Some(outdated) = &section.outdated {
                let _ = write!(md, " (outdated: {})", outdated.reasons());
            }
            md.push('\n');
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod set_apart_tests;
