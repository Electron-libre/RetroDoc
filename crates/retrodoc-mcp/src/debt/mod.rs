//! The documentation debt as typed items, independent of how it is shown
//! (`specs/debt-view.md`): the sections of the docs that need a human, read
//! from the same [`build_report`] as `retrodoc report`, plus the sections whose
//! cited code changed since the run, judged at each call.

use std::collections::BTreeMap;

use retrodoc_core::model::{SetApartReason, UseCase};
pub use retrodoc_pipeline::Section;
use retrodoc_pipeline::{build_report, DomainMap, WeakSection};

use crate::freshness::{FileState, Freshness};
use crate::read_model::GeneratedDocs;

/// What makes a section need a human, in the order the debt lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtKind {
    /// Confidence below `LOW_CONFIDENCE_THRESHOLD`.
    LowConfidence,
    /// A use case the confidence pass did not score.
    NotScored,
    /// A narrative the cited code does not back (`narrative_check` at 0).
    UnbackedNarrative,
    /// Business language below `LOW_CONFIDENCE_THRESHOLD`.
    TechnicalLanguage,
    /// Set apart under "Administration and technical" (`UseCase::is_technical`).
    SetApart,
    /// A file it cites was modified or deleted since the run.
    Stale,
}

impl DebtKind {
    pub const ALL: [Self; 6] = [
        Self::LowConfidence,
        Self::NotScored,
        Self::UnbackedNarrative,
        Self::TechnicalLanguage,
        Self::SetApart,
        Self::Stale,
    ];
}

/// A section of the docs that needs a human, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct DebtItem {
    pub kind: DebtKind,
    pub section: Section,
    /// The low confidence or business-language score.
    pub score: Option<f32>,
    pub rationale: Option<String>,
    /// Why a [`DebtKind::SetApart`] use case is set apart.
    pub reasons: Vec<SetApartReason>,
    /// The cited files of a [`DebtKind::Stale`] section that changed, in order.
    pub changed: Vec<(String, FileState)>,
}

impl DebtItem {
    fn new(kind: DebtKind, section: Section) -> Self {
        Self {
            kind,
            section,
            score: None,
            rationale: None,
            reasons: Vec::new(),
            changed: Vec::new(),
        }
    }

    fn weak(kind: DebtKind, weak: &WeakSection) -> Self {
        Self {
            score: Some(weak.score),
            rationale: weak.rationale.clone(),
            ..Self::new(kind, weak.section.clone())
        }
    }
}

/// The debt of a run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DebtList {
    /// By kind, then as `retrodoc report` lists them; the set apart and stale ones in the
    /// order of the run.
    pub items: Vec<DebtItem>,
    /// The run has use cases and none has a confidence: none is listed as not scored.
    pub no_score: bool,
}

/// The debt of `docs`, the stale sections judged now.
#[must_use]
pub fn debt(docs: &GeneratedDocs) -> DebtList {
    // The domain map only adds the code no domain holds, not listed here.
    let report = build_report(&DomainMap::default(), docs.features(), docs.use_cases());
    let no_score =
        !docs.use_cases().is_empty() && docs.use_cases().iter().all(|u| u.confidence.is_none());
    let mut items: Vec<DebtItem> = report
        .weak_sections
        .iter()
        .map(|w| DebtItem::weak(DebtKind::LowConfidence, w))
        .collect();
    if !no_score {
        items.extend(
            report
                .unscored_use_cases
                .iter()
                .map(|s| DebtItem::new(DebtKind::NotScored, s.clone())),
        );
    }
    items.extend(report.unbacked_narratives.iter().map(|w| DebtItem {
        // Always 0: the rationale says why.
        score: None,
        ..DebtItem::weak(DebtKind::UnbackedNarrative, w)
    }));
    items.extend(
        report
            .technical_use_cases
            .iter()
            .map(|w| DebtItem::weak(DebtKind::TechnicalLanguage, w)),
    );
    items.extend(report.set_apart_use_cases.into_iter().map(|s| DebtItem {
        reasons: s.reasons,
        ..DebtItem::new(DebtKind::SetApart, s.section)
    }));
    if let Some(freshness) = docs.freshness() {
        items.extend(stale(docs, freshness));
    }
    DebtList { items, no_score }
}

/// The files the steps of `use_case` cite, each once, in order.
#[must_use]
pub fn use_case_cited_paths(use_case: &UseCase) -> Vec<&str> {
    let mut cited: Vec<&str> = Vec::new();
    for source in use_case.steps.iter().flat_map(|s| &s.source_refs) {
        if !cited.contains(&source.path.as_str()) {
            cited.push(&source.path);
        }
    }
    cited
}

/// Each feature then its use cases, in the order of the run, whose cited files changed. Each file
/// is read once, however many sections cite it.
fn stale(docs: &GeneratedDocs, freshness: &Freshness) -> Vec<DebtItem> {
    let mut states: BTreeMap<String, FileState> = BTreeMap::new();
    let mut item = |section: Section, paths: Vec<&str>| {
        let changed: Vec<(String, FileState)> = paths
            .into_iter()
            .map(|path| {
                let state = *states
                    .entry(path.to_string())
                    .or_insert_with(|| freshness.state(path));
                (path.to_string(), state)
            })
            .filter(|(_, state)| matches!(state, FileState::Modified | FileState::Deleted))
            .collect();
        (!changed.is_empty()).then(|| DebtItem {
            changed,
            ..DebtItem::new(DebtKind::Stale, section)
        })
    };
    let mut items = Vec::new();
    for feature in docs.features() {
        let section = Section {
            domain: Some(feature.domain_slug.clone()),
            feature: feature.slug.clone(),
            use_case: None,
        };
        items.extend(item(
            section,
            feature.source_paths.iter().map(String::as_str).collect(),
        ));
        for use_case in docs.use_cases_of(&feature.slug) {
            let section = Section {
                domain: Some(feature.domain_slug.clone()),
                feature: feature.slug.clone(),
                use_case: Some(use_case.slug.clone()),
            };
            items.extend(item(section, use_case_cited_paths(use_case)));
        }
    }
    items
}

#[cfg(test)]
mod tests;
