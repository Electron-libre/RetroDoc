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
    BusinessRule, ConfidenceScore, Domain, Feature, RuleEvidence, SubDomain, UseCase,
    LOW_CONFIDENCE_THRESHOLD,
};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_ingest::signals::doc_sections;

use crate::behaviour_coverage::{BehaviourCoverage, EntryCount, FeatureCoverage};
use crate::domains::{DomainMap, UNCATEGORIZED_SLUG};
use crate::rules::{AsideFeature, AsideRule, SetAside};

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

/// A business rule that no existing document seems to mention.
#[derive(Debug, Clone, PartialEq)]
pub struct UndocumentedRule {
    pub slug: String,
    pub statement: String,
    /// Name of the feature the rule belongs to, if it still exists.
    pub feature: Option<String>,
}

/// How many business rules the project's own documents leave out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleDebt {
    pub total: usize,
    pub undocumented: Vec<UndocumentedRule>,
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
    /// The business rules the existing docs do not mention. `None` when no rule was stated
    /// (`[pipeline] rules` off, or nothing found); filled by the caller with
    /// [`DebtReport::with_rules`].
    pub rules: Option<RuleDebt>,
    /// What the rules pass set aside on a judgement of the LLM: to be checked. `None` when nothing.
    pub set_aside: Option<SetAside>,
}

/// Skipped files listed in the Markdown report (the rest is only counted).
const MAX_LISTED_SKIPPED: usize = 50;
/// Undocumented rules listed in the Markdown report (the rest is only counted).
const MAX_LISTED_RULES: usize = 50;
/// Technical support files listed in the Markdown report (the rest is only counted).
const MAX_LISTED_SUPPORT: usize = 50;
/// Entry points without a use case listed in the Markdown report (the rest is only counted).
const MAX_LISTED_UNCOVERED: usize = 50;

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0_f32, 0_u32), |(s, n), v| (s + v, n + 1));
    #[allow(clippy::cast_precision_loss)]
    (n > 0).then(|| sum / n as f32)
}

/// Converts the clustering into the rendering model: the "uncategorized"
/// bucket is left out (it belongs to the coverage report), and each domain /
/// sub-domain carries the mean confidence of its features.
#[must_use]
pub fn domain_models(map: &DomainMap, features: &[Feature]) -> Vec<Domain> {
    map.domains
        .iter()
        .filter(|d| d.slug != UNCATEGORIZED_SLUG)
        .map(|d| Domain {
            slug: d.slug.clone(),
            name: d.name.clone(),
            description: d.description.clone(),
            sub_domains: d
                .sub_domains
                .iter()
                .map(|s| SubDomain {
                    slug: s.slug.clone(),
                    name: s.name.clone(),
                    description: s.description.clone(),
                    confidence: mean_confidence(features.iter().filter(|f| {
                        f.domain_slug == d.slug && f.sub_domain_slug.as_deref() == Some(&s.slug)
                    })),
                })
                .collect(),
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
        set_apart_use_cases,
        back_office_features: back_office.len(),
        unscored_use_cases,
        uncategorized_files,
        skipped_files: Vec::new(),
        unplaced_entry_points: Vec::new(),
        entries_by_context: BTreeMap::new(),
        entries_by_feature: BTreeMap::new(),
        technical_support: Vec::new(),
        rules: None,
        set_aside: None,
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
    /// Adds what the behaviours leave out, from the saved coverage of a `surface` run.
    pub fn with_behaviour_coverage(&mut self, coverage: Option<BehaviourCoverage>) {
        if let Some(coverage) = coverage {
            self.unplaced_entry_points = coverage.unplaced_entry_points;
            self.entries_by_context = coverage.by_domain;
            self.entries_by_feature = coverage.by_feature;
            self.technical_support = coverage.technical_support;
        }
    }

    /// Adds the business rules that no existing document mentions: those whose statement shares at
    /// least half of its words with no section of `docs` (the project's own documents, not the
    /// generated ones) and that cite no document section themselves; a statement made of stop
    /// words only can't be judged and is not listed. Nothing when there is no rule.
    pub fn with_rules(
        &mut self,
        rules: &[BusinessRule],
        features: &[Feature],
        docs: &[ExistingDoc],
    ) {
        if rules.is_empty() {
            return;
        }
        let sections: Vec<String> = doc_sections(docs)
            .into_iter()
            // The path is not searched: a file named after a topic does not say what it states.
            .map(|s| s.text)
            .collect();
        let index = crate::bm25::Bm25::new(&sections);
        let undocumented = rules
            .iter()
            .filter(|r| {
                !r.evidence
                    .iter()
                    .any(|e| matches!(e, RuleEvidence::Doc { .. }))
                    && crate::bm25::has_content_words(&r.statement)
                    && index.search(&r.statement).is_empty()
            })
            .map(|r| UndocumentedRule {
                slug: r.slug.clone(),
                statement: r.statement.clone(),
                feature: features
                    .iter()
                    .find(|f| r.belongs_to_feature(&f.slug))
                    .map(|f| f.name.clone()),
            })
            .collect();
        self.rules = Some(RuleDebt {
            total: rules.len(),
            undocumented,
        });
    }

    /// Adds what the rules pass set aside on a judgement of the LLM. Nothing when it set nothing aside.
    pub fn with_set_aside(&mut self, set_aside: SetAside) {
        self.set_aside = (!set_aside.is_empty()).then_some(set_aside);
    }

    /// The section on what the rules pass set aside: the features taken for tests (they lost every
    /// rule, so a wrong answer costs the most) and the rules dropped as noise.
    fn write_set_aside_section(&self, md: &mut String) {
        let Some(aside) = &self.set_aside else { return };
        md.push_str(
            "\n## Set aside by the rules pass (to check)\n\nThe LLM judged these; a wrong answer \
             hides a business rule. The full list is in `.retrodoc/cache/rules-set-aside.yaml`, \
             every rule as each feature stated it in `.retrodoc/cache/rules-stated.yaml`.\n",
        );
        if !aside.features.is_empty() {
            let _ = write!(
                md,
                "\n### {} feature(s) without rules\n\nTaken for features about tests, so no rule was \
                 asked of them; if one of them is a business feature, its rules are missing:\n\n",
                aside.features.len()
            );
            for AsideFeature {
                slug,
                name,
                description,
                reason,
            } in aside.features.iter().take(MAX_LISTED_RULES)
            {
                let _ = writeln!(
                    md,
                    "- `{slug}` {}: {} ({})",
                    one_line(name),
                    one_line(description),
                    one_line(reason)
                );
            }
            if aside.features.len() > MAX_LISTED_RULES {
                let _ = writeln!(
                    md,
                    "- … and {} more",
                    aside.features.len() - MAX_LISTED_RULES
                );
            }
        }
        if !aside.rules.is_empty() {
            let _ = write!(
                md,
                "\n### {} rule(s) dropped as noise\n\n",
                aside.rules.len()
            );
            for AsideRule {
                slug,
                statement,
                class,
                ..
            } in aside.rules.iter().take(MAX_LISTED_RULES)
            {
                let _ = writeln!(md, "- `{slug}` ({class}) {}", one_line(statement));
            }
            if aside.rules.len() > MAX_LISTED_RULES {
                let _ = writeln!(md, "- … and {} more", aside.rules.len() - MAX_LISTED_RULES);
            }
        }
    }

    /// The section on the business rules the existing docs leave out.
    fn write_rule_section(&self, md: &mut String) {
        let Some(debt) = &self.rules else { return };
        let _ = write!(
            md,
            "\n## Business rules without documentation\n\n{} of {} business rule(s) are \
             mentioned in no existing document",
            debt.undocumented.len(),
            debt.total
        );
        if debt.undocumented.is_empty() {
            md.push_str(".\n");
            return;
        }
        md.push_str(":\n\n");
        for rule in debt.undocumented.iter().take(MAX_LISTED_RULES) {
            let place = rule
                .feature
                .as_ref()
                .map_or_else(String::new, |f| format!(" — {f}"));
            let _ = writeln!(md, "- `{}` {}{place}", rule.slug, rule.statement);
        }
        if debt.undocumented.len() > MAX_LISTED_RULES {
            let _ = writeln!(
                md,
                "- … and {} more",
                debt.undocumented.len() - MAX_LISTED_RULES
            );
        }
    }

    /// The sections on what the behaviours leave out (`surface` runs only).
    fn write_behaviour_sections(&self, md: &mut String) {
        if !self.unplaced_entry_points.is_empty() {
            md.push_str(
                "\n## Entry points without a feature\n\nNo feature holds these entry points, so \
                 their behaviour is not documented:\n\n",
            );
            for id in &self.unplaced_entry_points {
                let _ = writeln!(md, "- `{id}`");
            }
        }
        if !self.entries_by_context.is_empty() {
            md.push_str(
                "\n## Entry points by context\n\n| Context | Entry points | Without a feature |\n|---|---|---|\n",
            );
            let mut rows: Vec<_> = self.entries_by_context.iter().collect();
            rows.sort_by_key(|(_, count)| std::cmp::Reverse((count.unplaced, count.entry_points)));
            for (context, count) in rows {
                let _ = writeln!(
                    md,
                    "| {context} | {} | {} |",
                    count.entry_points, count.unplaced
                );
            }
        }
        self.write_uncovered_by_use_cases(md);
        if !self.technical_support.is_empty() {
            let _ = write!(
                md,
                "\n## Technical support (reached by no behaviour)\n\n{} source file(s) hold no \
                 entry point of a feature, are not run by one and mention no entity; they are \
                 absent from the functional docs:\n\n",
                self.technical_support.len()
            );
            for path in self.technical_support.iter().take(MAX_LISTED_SUPPORT) {
                let _ = writeln!(md, "- `{}`", path.display());
            }
            if self.technical_support.len() > MAX_LISTED_SUPPORT {
                let _ = writeln!(
                    md,
                    "- … and {} more",
                    self.technical_support.len() - MAX_LISTED_SUPPORT
                );
            }
        }
    }

    /// The entry points of the features that no use case cites, per feature and overall.
    fn write_uncovered_by_use_cases(&self, md: &mut String) {
        let total: usize = self
            .entries_by_feature
            .values()
            .map(|c| c.entry_points)
            .sum();
        let uncovered: usize = self
            .entries_by_feature
            .values()
            .map(|c| c.uncovered.len())
            .sum();
        if uncovered == 0 {
            return;
        }
        let _ = write!(
            md,
            "\n## Entry points without a use case\n\n{uncovered} of {total} entry points held by \
             a feature are cited by no use case, so what they do is not described:\n\n\
             | Feature | Entry points | Without a use case |\n|---|---|---|\n"
        );
        let mut rows: Vec<_> = self
            .entries_by_feature
            .iter()
            .filter(|(_, c)| !c.uncovered.is_empty())
            .collect();
        rows.sort_by_key(|(_, c)| std::cmp::Reverse((c.uncovered.len(), c.entry_points)));
        for (feature, count) in &rows {
            let _ = writeln!(
                md,
                "| {feature} | {} | {} |",
                count.entry_points,
                count.uncovered.len()
            );
        }
        md.push('\n');
        let ids: Vec<&String> = rows.iter().flat_map(|(_, c)| &c.uncovered).collect();
        for id in ids.iter().take(MAX_LISTED_UNCOVERED) {
            let _ = writeln!(md, "- `{id}`");
        }
        if ids.len() > MAX_LISTED_UNCOVERED {
            let _ = writeln!(md, "- … and {} more", ids.len() - MAX_LISTED_UNCOVERED);
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
        self.write_rule_section(&mut md);
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
mod tests {
    use super::*;

    use crate::domains::DomainCluster;

    fn feature(slug: &str, domain: &str, score: Option<f32>) -> Feature {
        Feature {
            back_office: false,
            slug: slug.to_string(),
            domain_slug: domain.to_string(),
            sub_domain_slug: None,
            name: slug.to_string(),
            description: String::new(),
            source_paths: vec![],
            entry_points: Vec::new(),
            confidence: score.map(|s| ConfidenceScore::new(s, None)),
        }
    }

    fn use_case(slug: &str, feature: &str, score: Option<f32>) -> UseCase {
        UseCase {
            entry_points: Vec::new(),
            primary_actor: None,
            narrative: None,
            business_language: None,
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
    fn the_behaviour_coverage_lists_unplaced_entry_points_and_technical_support() {
        let mut report = build_report(&DomainMap::default(), &[], &[]);
        assert!(!report.to_markdown().contains("Technical support"));

        report.with_behaviour_coverage(Some(BehaviourCoverage {
            unplaced_entry_points: vec!["a.rb#GET /a".to_string()],
            technical_support: vec![PathBuf::from("helper.rb")],
            by_domain: BTreeMap::from([
                (
                    "orders".to_string(),
                    EntryCount {
                        entry_points: 40,
                        unplaced: 3,
                    },
                ),
                (
                    "billing".to_string(),
                    EntryCount {
                        entry_points: 5,
                        unplaced: 0,
                    },
                ),
            ]),
            by_feature: BTreeMap::from([
                (
                    "sign".to_string(),
                    FeatureCoverage {
                        entry_points: 4,
                        uncovered: vec!["a.rb#GET /b".to_string(), "a.rb#GET /c".to_string()],
                    },
                ),
                (
                    "pay".to_string(),
                    FeatureCoverage {
                        entry_points: 2,
                        uncovered: Vec::new(),
                    },
                ),
            ]),
        }));

        let md = report.to_markdown();
        assert!(md.contains("## Entry points without a use case"), "{md}");
        assert!(md.contains("2 of 6 entry points"), "{md}");
        assert!(
            md.contains("| sign | 4 | 2 |") && !md.contains("| pay |"),
            "{md}"
        );
        assert!(md.contains("`a.rb#GET /c`"));
        assert!(md.contains("## Entry points without a feature"), "{md}");
        assert!(md.contains("`a.rb#GET /a`"));
        assert!(
            md.contains("| orders | 40 | 3 |") && md.contains("| billing | 5 | 0 |"),
            "{md}"
        );
        assert!(
            md.find("| orders |") < md.find("| billing |"),
            "the largest gap first"
        );
        assert!(md.contains("## Technical support (reached by no behaviour)"));
        assert!(md.contains("1 source file(s)"));
        assert!(md.contains("`helper.rb`"));
    }

    fn rule(slug: &str, statement: &str, evidence: Vec<RuleEvidence>) -> BusinessRule {
        BusinessRule {
            slug: slug.to_string(),
            statement: statement.to_string(),
            kind: retrodoc_core::model::RuleKind::Limit,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence,
            confidence: None,
        }
    }

    #[test]
    fn rules_no_existing_doc_mentions_are_counted_and_listed() {
        let docs = [ExistingDoc {
            path: PathBuf::from("README.md"),
            content: "# Orders\nAn order is refused when delivery takes an hour.\n".into(),
        }];
        let features = [feature("orders", "billing", None)];
        let rules = [
            rule(
                "orders-1",
                "An order is refused when delivery takes an hour or more.",
                vec![],
            ),
            rule(
                "orders-2",
                "Riders above the weekly cap are paused.",
                vec![],
            ),
            rule(
                "orders-3",
                "Cooking time counts before the rider leaves.",
                vec![RuleEvidence::Doc {
                    path: "docs/x.md".into(),
                    heading: "Time".into(),
                }],
            ),
            rule("gone-1", "Credits expire after a year.", vec![]),
        ];
        let mut report = build_report(&DomainMap::default(), &[], &[]);
        assert!(!report.to_markdown().contains("Business rules without"));

        report.with_rules(&rules, &features, &docs);

        let debt = report.rules.as_ref().unwrap();
        assert_eq!(debt.total, 4);
        let slugs: Vec<&str> = debt.undocumented.iter().map(|r| r.slug.as_str()).collect();
        assert_eq!(slugs, ["orders-2", "gone-1"]);
        assert_eq!(debt.undocumented[1].feature, None);
        let md = report.to_markdown();
        assert!(
            md.contains("## Business rules without documentation"),
            "{md}"
        );
        assert!(
            md.contains("2 of 4 business rule(s) are mentioned in no existing document"),
            "{md}"
        );
        assert!(
            md.contains("- `orders-2` Riders above the weekly cap are paused. — orders"),
            "{md}"
        );
        assert!(!md.contains("`orders-1`"), "{md}");
    }

    #[test]
    fn a_file_named_after_the_topic_or_a_statement_of_stop_words_is_not_judged_wrongly() {
        let docs = [ExistingDoc {
            path: PathBuf::from("docs/rider-cap.md"),
            content: "Nothing about it here.".into(),
        }];
        let mut report = build_report(&DomainMap::default(), &[], &[]);
        report.with_rules(
            &[
                rule("a-1", "Riders above the cap are paused.", vec![]),
                rule("a-2", "It is what it is.", vec![]),
            ],
            &[],
            &docs,
        );
        let slugs: Vec<&str> = report
            .rules
            .as_ref()
            .unwrap()
            .undocumented
            .iter()
            .map(|r| r.slug.as_str())
            .collect();
        assert_eq!(slugs, ["a-1"]);
    }

    #[test]
    fn no_rule_adds_no_section_and_documented_rules_say_so() {
        let mut report = build_report(&DomainMap::default(), &[], &[]);
        report.with_rules(&[], &[], &[]);
        assert!(report.rules.is_none());

        let docs = [ExistingDoc {
            path: PathBuf::from("README.md"),
            content: "Riders above the weekly cap are paused.".into(),
        }];
        report.with_rules(
            &[rule(
                "a-1",
                "Riders above the weekly cap are paused.",
                vec![],
            )],
            &[],
            &docs,
        );
        let md = report.to_markdown();
        assert!(
            md.contains("0 of 1 business rule(s) are mentioned in no existing document.\n"),
            "{md}"
        );
    }

    #[test]
    fn what_the_rules_pass_set_aside_is_listed_to_be_checked() {
        let mut report = build_report(&DomainMap::default(), &[], &[]);
        report.with_set_aside(SetAside::default());
        assert!(!report.to_markdown().contains("Set aside by the rules pass"));

        report.with_set_aside(SetAside {
            features: vec![AsideFeature {
                slug: "email-verification".into(),
                name: "Verify an email".into(),
                description: "Checks the address\nof a new user".into(),
                reason: "describes how the application is tested".into(),
            }],
            rules: vec![AsideRule {
                slug: "orders-1".into(),
                feature: "orders".into(),
                statement: "An order is stored by the customer id.".into(),
                class: "storage".into(),
            }],
        });
        let md = report.to_markdown();
        assert!(md.contains("## Set aside by the rules pass"), "{md}");
        assert!(md.contains("`email-verification`"), "{md}");
        assert!(md.contains("Checks the address of a new user"), "{md}");
        assert!(
            md.contains("describes how the application is tested"),
            "{md}"
        );
        assert!(md.contains("`orders-1` (storage)"), "{md}");
        assert!(md.contains("rules-set-aside.yaml"), "{md}");
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
                ..DomainCluster::default()
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

        let mut report = report;
        report.skipped_files = vec![PathBuf::from("left.rs")];
        assert!(report
            .to_markdown()
            .contains("## Not analysed (file budget)"));
        assert!(report.to_markdown().contains("`left.rs`"));

        let md = report.to_markdown();
        assert!(md.contains("Overall confidence: **55%**"));
        assert!(md.contains("`billing/pay/pay-invoice` — 20%: because"));
        assert!(md.contains("`x.rs`"));
    }

    #[test]
    fn technical_sounding_use_cases_get_their_own_section() {
        let mut readable = use_case("readable", "pay", Some(0.9));
        readable.business_language = Some(ConfidenceScore::new(0.9, None));
        let mut technical = use_case("helper", "pay", Some(0.9));
        technical.business_language = Some(ConfidenceScore::new(
            0.2,
            "code-level wording in the narrative: controller".to_string(),
        ));
        let features = [feature("pay", "billing", Some(0.9))];

        let report = build_report(&DomainMap::default(), &features, &[readable, technical]);

        assert_eq!(report.technical_use_cases.len(), 1);
        assert_eq!(report.technical_use_cases[0].label, "billing/pay/helper");
        // 0.2 is under both the debt threshold and the one that sets a use case apart.
        assert_eq!(
            (report.set_apart_use_cases, report.back_office_features),
            (1, 0)
        );
        assert!((report.overall_business.unwrap() - 0.55).abs() < 1e-6);
        let md = report.to_markdown();
        assert!(md.contains("Overall business language: **55%**"));
        assert!(md.contains("## Technical-sounding use cases (business language below 50%)"));
        assert!(md.contains(
            "`billing/pay/helper` — 20%: code-level wording in the narrative: controller"
        ));
        // Not scored (older artifacts): no business section at all.
        let plain = build_report(
            &DomainMap::default(),
            &features,
            &[use_case("a", "pay", Some(0.9))],
        );
        assert!(!plain.to_markdown().contains("business language"));
    }
}

#[cfg(test)]
mod set_apart_tests {
    use super::*;

    #[test]
    fn the_report_counts_what_the_docs_set_apart() {
        let mut admin = Feature {
            slug: "admin-tools".into(),
            domain_slug: "ops".into(),
            sub_domain_slug: None,
            name: "Admin".into(),
            description: String::new(),
            source_paths: Vec::new(),
            entry_points: Vec::new(),
            back_office: true,
            confidence: None,
        };
        let mut business = admin.clone();
        business.slug = "pay".into();
        business.back_office = false;
        admin.back_office = true;
        let uc = |slug: &str, feature: &str, business_language: f32| UseCase {
            slug: slug.into(),
            feature_slug: feature.into(),
            name: slug.into(),
            description: String::new(),
            steps: Vec::new(),
            entry_points: Vec::new(),
            primary_actor: None,
            narrative: None,
            business_language: Some(ConfidenceScore::new(business_language, None)),
            diagram_mermaid: None,
            confidence: None,
        };
        let use_cases = [
            uc("reimport", "admin-tools", 0.95),
            uc("pay", "pay", 0.95),
            uc("helper", "pay", 0.7),
        ];

        let report = build_report(&DomainMap::default(), &[admin, business], &use_cases);

        assert_eq!(
            (report.set_apart_use_cases, report.back_office_features),
            (2, 1)
        );
        assert!(report
            .to_markdown()
            .contains("2 use case(s) and 1 back-office feature(s)"));
        let none = build_report(&DomainMap::default(), &[], &[uc("pay", "pay", 0.95)]);
        assert!(!none.to_markdown().contains("Set apart in the docs"));
    }
}
