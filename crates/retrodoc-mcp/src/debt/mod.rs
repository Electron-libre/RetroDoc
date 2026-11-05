//! The documentation debt as typed items, independent of how it is shown
//! (`specs/debt-view.md`): the sections of the docs that need a human, read
//! from the same [`build_report`] as `retrodoc report`, plus the sections whose
//! cited code changed since the run, judged at each call, then the items the
//! project's docs contradict or leave out, the sections a human marked wrong or
//! to review and the code no doc covers, by folder, a section a human validated
//! left out of the kinds that judge it, sorted by a priority that crosses the weight of its kind and how weak each
//! item is with the commits of its busiest file and its entry points.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use retrodoc_core::model::{RuleEvidence, SetApartReason, UseCase};
pub use retrodoc_pipeline::reconcile::{Contradiction, ItemKind};
use retrodoc_pipeline::reconcile::{ItemVerdict, Reconciliation};
pub use retrodoc_pipeline::Section;
use retrodoc_pipeline::{build_report, DebtReport, DomainMap, WeakSection};

use crate::edits;
use crate::freshness::{FileState, Freshness};
use crate::read_model::{GeneratedDocs, Uncovered};
use crate::reviews::{self, Outdated, Reviews, SectionKind, Status};

/// What makes a section need a human, in the order the debt lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    /// An item a section of the project's docs contradicts.
    Contradicted,
    /// An item no section of the project's docs mentions.
    Undocumented,
    /// A domain, feature or use case a human marked wrong.
    MarkedWrong,
    /// A domain, feature or use case a human left to review.
    ToReview,
    /// A domain, feature or use case whose human edit no longer corrects it as generated now.
    OutdatedEdit,
    /// The entry points of a folder that no feature holds.
    EntryPointWithoutFeature,
    /// The source files of a folder the clustering left in no domain.
    NoDomain,
    /// The source files of a folder no behaviour reaches.
    NoBehaviour,
    /// The source files of a folder the file budget left out.
    NotAnalysed,
}

impl DebtKind {
    pub const ALL: [Self; 15] = [
        Self::LowConfidence,
        Self::NotScored,
        Self::UnbackedNarrative,
        Self::TechnicalLanguage,
        Self::SetApart,
        Self::Stale,
        Self::Contradicted,
        Self::Undocumented,
        Self::MarkedWrong,
        Self::ToReview,
        Self::OutdatedEdit,
        Self::EntryPointWithoutFeature,
        Self::NoDomain,
        Self::NoBehaviour,
        Self::NotAnalysed,
    ];

    /// The kinds whose items carry a domain, in the order of [`Self::ALL`].
    pub const WITH_DOMAIN: [Self; 11] = [
        Self::LowConfidence,
        Self::NotScored,
        Self::UnbackedNarrative,
        Self::TechnicalLanguage,
        Self::SetApart,
        Self::Stale,
        Self::Contradicted,
        Self::Undocumented,
        Self::MarkedWrong,
        Self::ToReview,
        Self::OutdatedEdit,
    ];

    /// The value of `?kind=` of the debt page, also the name of the kind in the run history.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::LowConfidence => "low_confidence",
            Self::NotScored => "not_scored",
            Self::UnbackedNarrative => "unbacked_narrative",
            Self::TechnicalLanguage => "technical_language",
            Self::SetApart => "set_apart",
            Self::Stale => "stale",
            Self::Contradicted => "contradicted",
            Self::Undocumented => "undocumented",
            Self::MarkedWrong => "marked_wrong",
            Self::ToReview => "to_review",
            Self::OutdatedEdit => "outdated_edit",
            Self::EntryPointWithoutFeature => "entry_point_without_feature",
            Self::NoDomain => "no_domain",
            Self::NoBehaviour => "no_behaviour",
            Self::NotAnalysed => "not_analysed",
        }
    }

    /// What an item of this kind weighs in its priority (DBT-29): 1 when its section may state
    /// what is false, 0.5 when it is missing, unreadable or left to check, 0.25 when the docs leave
    /// it out by design.
    #[must_use]
    pub fn weight(self) -> f32 {
        match self {
            Self::LowConfidence
            | Self::UnbackedNarrative
            | Self::Stale
            | Self::Contradicted
            | Self::MarkedWrong => 1.0,
            Self::NotScored
            | Self::TechnicalLanguage
            | Self::Undocumented
            | Self::EntryPointWithoutFeature
            | Self::ToReview
            | Self::OutdatedEdit => 0.5,
            Self::SetApart | Self::NoDomain | Self::NoBehaviour | Self::NotAnalysed => 0.25,
        }
    }
}

impl DebtKind {
    /// The kinds that judge a section, which a human validation of it removes (DBT-32).
    fn judges(self) -> bool {
        matches!(
            self,
            Self::LowConfidence
                | Self::NotScored
                | Self::UnbackedNarrative
                | Self::TechnicalLanguage
        )
    }
}

/// What a debt item is about.
#[derive(Debug, Clone, PartialEq)]
pub enum Subject {
    /// A use case, or a feature.
    Section(Section),
    /// A domain of the run, by its slug.
    Domain(String),
    /// A domain, feature, use case or rule reconciled with the project's docs.
    DocItem(DocItem),
    /// A folder of the code no doc covers.
    Folder(Folder),
}

/// An item of the reconciliation with the project's docs.
#[derive(Debug, Clone, PartialEq)]
pub struct DocItem {
    pub kind: ItemKind,
    /// `<domain>/<feature>/<use case or rule>`, `?` for a domain or feature not in the run.
    pub label: String,
    pub domain: String,
    pub name: String,
    pub entry_points: usize,
    /// The sections that contradict it, and what each side states.
    pub contradictions: Vec<Contradiction>,
}

impl DocItem {
    fn of(verdict: &ItemVerdict) -> Self {
        Self {
            kind: verdict.kind,
            label: verdict.label.clone(),
            domain: verdict.domain.clone(),
            name: verdict.name.clone(),
            entry_points: verdict.entry_points,
            contradictions: verdict.contradictions.clone(),
        }
    }
}

/// A folder and its files or entry points (`<file>#<name>`) of one kind, in the order of the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    /// The directory of its files, `.` for the root of the repository.
    pub path: String,
    pub entries: Vec<String>,
}

/// A section of the docs that needs a human, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct DebtItem {
    pub kind: DebtKind,
    pub subject: Subject,
    /// The low confidence or business-language score.
    pub score: Option<f32>,
    pub rationale: Option<String>,
    /// Why a [`DebtKind::SetApart`] use case is set apart.
    pub reasons: Vec<SetApartReason>,
    /// The cited files of a [`DebtKind::Stale`] section that changed, in order.
    pub changed: Vec<(String, FileState)>,
    /// Why the review of a [`DebtKind::MarkedWrong`] or [`DebtKind::ToReview`] section no longer
    /// vouches for it as it is now, or the edit of a [`DebtKind::OutdatedEdit`] one no longer
    /// corrects it as generated now.
    pub outdated: Option<Outdated>,
    pub priority: Priority,
}

/// What fixing an item first is worth, and its parts (`specs/debt-view.md`, DBT-23).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Priority {
    /// The weight of its kind ([`DebtKind::weight`]).
    pub weight: f32,
    /// `1 − score` for an item with a score, else 1.
    pub weakness: f32,
    /// The commits of the history that changed the busiest of its files.
    pub commits: u32,
    pub entry_points: usize,
}

impl Default for Priority {
    fn default() -> Self {
        Self {
            weight: 1.0,
            weakness: 1.0,
            commits: 0,
            entry_points: 0,
        }
    }
}

impl Priority {
    /// `weight × weakness × (1 + ln(1 + commits)) × (1 + ln(1 + entry points))`.
    #[must_use]
    pub fn value(&self) -> f64 {
        let entry_points = u32::try_from(self.entry_points).unwrap_or(u32::MAX);
        f64::from(self.weight)
            * f64::from(self.weakness)
            * (1.0 + f64::from(self.commits).ln_1p())
            * (1.0 + f64::from(entry_points).ln_1p())
    }
}

impl DebtItem {
    fn new(kind: DebtKind, section: Section) -> Self {
        Self::about(kind, Subject::Section(section))
    }

    fn about(kind: DebtKind, subject: Subject) -> Self {
        Self {
            kind,
            subject,
            score: None,
            rationale: None,
            reasons: Vec::new(),
            changed: Vec::new(),
            outdated: None,
            priority: Priority::default(),
        }
    }

    fn weak(kind: DebtKind, weak: &WeakSection) -> Self {
        Self {
            score: Some(weak.score),
            rationale: weak.rationale.clone(),
            ..Self::new(kind, weak.section.clone())
        }
    }

    /// The domain of its section or doc item; `None` for a folder, or a domain not in the run.
    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        match &self.subject {
            Subject::Section(section) => section.domain.as_deref(),
            Subject::Domain(slug) => Some(slug),
            Subject::DocItem(item) => Some(item.domain.as_str()).filter(|d| *d != "?"),
            Subject::Folder(_) => None,
        }
    }

    /// The label of its section or doc item, or the path of its folder.
    #[must_use]
    pub fn label(&self) -> String {
        match &self.subject {
            Subject::Section(section) => section.label(),
            Subject::Domain(slug) => slug.clone(),
            Subject::DocItem(item) => item.label.clone(),
            Subject::Folder(folder) => folder.path.clone(),
        }
    }
}

/// The debt of a run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DebtList {
    /// The highest priority first; equal priorities by kind, then as `retrodoc report` lists
    /// them, the set apart and stale ones in the order of the run.
    pub items: Vec<DebtItem>,
    /// The run has use cases and none has a confidence: none is listed as not scored.
    pub no_score: bool,
    /// There is no collected doc to compare the items with: none is listed as contradicted or
    /// undocumented.
    pub no_docs: bool,
}

/// The debt of `docs` with its `reviews`, the stale sections judged now.
#[must_use]
pub fn debt(docs: &GeneratedDocs, reviews: &Reviews) -> DebtList {
    debt_of(docs, docs_report(docs), reviews)
}

/// The debt report of `docs`, as [`debt`] reads it.
#[must_use]
pub fn docs_report(docs: &GeneratedDocs) -> DebtReport {
    // The domain map only adds the code no domain holds, not listed here.
    build_report(&DomainMap::default(), docs.features(), docs.use_cases())
}

/// The debt of `docs` from its `report` ([`docs_report`]) and its `reviews`, the stale sections
/// judged now.
#[must_use]
pub fn debt_of(docs: &GeneratedDocs, report: DebtReport, reviews: &Reviews) -> DebtList {
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
    let uncovered = docs.uncovered();
    if let Some(reconciliation) = &uncovered.reconciliation {
        items.extend(doc_items(reconciliation));
    }
    let validated = validated(docs, reviews);
    items.retain(|item| {
        !(item.kind.judges()
            && matches!(&item.subject, Subject::Section(s) if validated.contains(&review_section(s))))
    });
    items.extend(reviewed(docs, reviews));
    items.extend(outdated_edits(docs));
    items.extend(code_items(uncovered));
    for item in &mut items {
        item.priority = priority(docs, item);
    }
    // Stable: equal priorities keep the order by kind.
    items.sort_by(|a, b| b.priority.value().total_cmp(&a.priority.value()));
    DebtList {
        items,
        no_score,
        no_docs: uncovered.reconciliation.is_none(),
    }
}

/// The priority of `item`: the weight of its kind, its weakness, the commits of its busiest file and
/// its entry points.
fn priority(docs: &GeneratedDocs, item: &DebtItem) -> Priority {
    let (files, entry_points) = match &item.subject {
        Subject::Section(section) => section_parts(docs, section),
        Subject::Domain(slug) => domain_parts(docs, slug),
        Subject::DocItem(doc_item) => (doc_item_files(docs, doc_item), doc_item.entry_points),
        Subject::Folder(folder) => (
            folder
                .entries
                .iter()
                .filter_map(|e| entry_file(e))
                .collect(),
            if item.kind == DebtKind::EntryPointWithoutFeature {
                folder.entries.len()
            } else {
                0
            },
        ),
    };
    let commits = docs.churn().map_or(0, |churn| {
        files
            .into_iter()
            .filter_map(|path| churn.get(path).copied())
            .max()
            .unwrap_or(0)
    });
    Priority {
        weight: item.kind.weight(),
        weakness: item.score.map_or(1.0, |score| 1.0 - score),
        commits,
        entry_points,
    }
}

/// The files and the number of entry points of a use case, or of a feature; none when its feature
/// is not in the run.
fn section_parts<'a>(docs: &'a GeneratedDocs, section: &Section) -> (Vec<&'a str>, usize) {
    let Some(feature) = docs.features().iter().find(|f| f.slug == section.feature) else {
        return (Vec::new(), 0);
    };
    let entry_points = match &section.use_case {
        Some(slug) => docs
            .use_cases()
            .iter()
            .find(|u| u.feature_slug == feature.slug && u.slug == *slug)
            .map_or(0, |u| u.entry_points.len()),
        None => feature.entry_points.len(),
    };
    (
        reviews::cited_paths(docs, &review_section(section)),
        entry_points,
    )
}

/// The files of the features of a domain, and the sum of their entry points.
fn domain_parts<'a>(docs: &'a GeneratedDocs, slug: &str) -> (Vec<&'a str>, usize) {
    (
        reviews::cited_paths(docs, &reviews::Section::new(SectionKind::Domain, slug)),
        docs.features()
            .iter()
            .filter(|f| f.domain_slug == slug)
            .map(|f| f.entry_points.len())
            .sum(),
    )
}

/// The section of the reviews (REV-1) a feature or use case of the debt names.
fn review_section(section: &Section) -> reviews::Section {
    match &section.use_case {
        Some(use_case) => reviews::Section::use_case(&section.feature, use_case),
        None => reviews::Section::new(SectionKind::Feature, &section.feature),
    }
}

/// The sections a human validated whose review is not outdated.
fn validated<'a>(docs: &GeneratedDocs, reviews: &'a Reviews) -> HashSet<&'a reviews::Section> {
    reviews
        .reviews()
        .iter()
        .filter(|r| r.status == Status::Validated && reviews::outdated(docs, r).is_none())
        .map(|r| &r.section)
        .collect()
}

/// The sections of the run marked wrong, then those left to review, as the reviews are saved; the
/// comment is the reason. `retrodoc report` lists the ones marked wrong from them.
pub fn reviewed<'a>(
    docs: &'a GeneratedDocs,
    reviews: &'a Reviews,
) -> impl Iterator<Item = DebtItem> + 'a {
    let of = move |kind: DebtKind, status: Status| {
        reviews
            .reviews()
            .iter()
            .filter(move |r| r.status == status)
            .filter_map(move |r| {
                // The subject first: no file read for a section the run no longer holds.
                let subject = section_subject(docs, &r.section)?;
                Some(DebtItem {
                    rationale: Some(r.comment.clone()).filter(|c| !c.trim().is_empty()),
                    outdated: reviews::outdated(docs, r),
                    ..DebtItem::about(kind, subject)
                })
            })
    };
    of(DebtKind::MarkedWrong, Status::Wrong).chain(of(DebtKind::ToReview, Status::ToReview))
}

/// The sections of the run whose edit is outdated, as the edits are saved (DBT-39).
fn outdated_edits(docs: &GeneratedDocs) -> impl Iterator<Item = DebtItem> + '_ {
    docs.edits().edits().iter().filter_map(move |edit| {
        // The subject first: no file read for a section the run no longer holds.
        let subject = section_subject(docs, &edit.section)?;
        Some(DebtItem {
            outdated: Some(edits::outdated(docs, edit)?),
            ..DebtItem::about(DebtKind::OutdatedEdit, subject)
        })
    })
}

/// The subject of the section a review or an edit names; `None` when the run no longer holds it.
fn section_subject(docs: &GeneratedDocs, section: &reviews::Section) -> Option<Subject> {
    let id = section.id.as_str();
    let feature_section = |feature: &str, use_case: Option<&str>| {
        let feature = docs.features().iter().find(|f| f.slug == feature)?;
        Some(Subject::Section(Section {
            domain: Some(feature.domain_slug.clone()),
            feature: feature.slug.clone(),
            use_case: use_case.map(str::to_string),
        }))
    };
    match section.kind {
        SectionKind::Domain => docs
            .domains()
            .iter()
            .any(|d| d.slug == id)
            .then(|| Subject::Domain(id.to_string())),
        SectionKind::Feature => feature_section(id, None),
        SectionKind::UseCase => {
            let (feature, slug) = id.split_once('/')?;
            docs.use_cases_of(feature)
                .any(|u| u.slug == slug)
                .then(|| feature_section(feature, Some(slug)))?
        }
    }
}

/// The files of a reconciled item, from its label `<domain>/<feature>/<use case or rule>`; none
/// when its feature is no longer in the run (`?`).
fn doc_item_files<'a>(docs: &'a GeneratedDocs, item: &DocItem) -> Vec<&'a str> {
    let parts: Vec<&str> = item.label.split('/').collect();
    let section = |feature: &str, use_case: Option<&str>| Section {
        domain: Some(item.domain.clone()),
        feature: feature.to_string(),
        use_case: use_case.map(str::to_string),
    };
    match (item.kind, parts.as_slice()) {
        (_, ["?", ..]) => Vec::new(),
        (ItemKind::Domain, [domain]) => docs
            .features()
            .iter()
            .filter(|f| f.domain_slug == *domain)
            .flat_map(|f| f.source_paths.iter().map(String::as_str))
            .collect(),
        (ItemKind::Feature, [_, feature]) => section_parts(docs, &section(feature, None)).0,
        (ItemKind::UseCase, [_, feature, use_case]) => {
            section_parts(docs, &section(feature, Some(use_case))).0
        }
        (ItemKind::Rule, [_, _, rule]) => docs
            .rules()
            .iter()
            .filter(|r| r.slug == *rule)
            .flat_map(|r| &r.evidence)
            .filter_map(|e| match e {
                RuleEvidence::Code { source } => Some(source.path.as_str()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The contradicted items in the order of the run, then the undocumented ones as `retrodoc report`
/// lists them.
fn doc_items(reconciliation: &Reconciliation) -> impl Iterator<Item = DebtItem> + '_ {
    let of = |kind: DebtKind| {
        move |verdict: &ItemVerdict| DebtItem::about(kind, Subject::DocItem(DocItem::of(verdict)))
    };
    let contradicted = reconciliation.contradicted();
    let undocumented = reconciliation.undocumented();
    contradicted
        .into_iter()
        .map(of(DebtKind::Contradicted))
        .chain(undocumented.into_iter().map(of(DebtKind::Undocumented)))
}

/// The code no doc covers, one item per kind and folder, the folders by path.
fn code_items(uncovered: &Uncovered) -> Vec<DebtItem> {
    let coverage = uncovered.coverage.as_ref();
    let display = |paths: &[std::path::PathBuf]| -> Vec<String> {
        paths.iter().map(|p| p.display().to_string()).collect()
    };
    let kinds = [
        (
            DebtKind::EntryPointWithoutFeature,
            coverage.map(|c| c.unplaced_entry_points.clone()),
        ),
        (DebtKind::NoDomain, Some(display(&uncovered.no_domain))),
        (
            DebtKind::NoBehaviour,
            coverage.map(|c| display(&c.technical_support)),
        ),
        (DebtKind::NotAnalysed, Some(display(&uncovered.skipped))),
    ];
    let mut items = Vec::new();
    for (kind, entries) in kinds {
        let mut folders: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for entry in entries.unwrap_or_default() {
            folders.entry(folder_of(&entry)).or_default().push(entry);
        }
        items.extend(folders.into_iter().map(|(path, entries)| {
            DebtItem::about(kind, Subject::Folder(Folder { path, entries }))
        }));
    }
    items
}

/// The file of `entry`: a path, or an entry point `<file>#<name>`; `None` for a `Class#method`
/// saved before ids, which names no file (`retrodoc_core::model::entry_point_name`).
#[must_use]
pub fn entry_file(entry: &str) -> Option<&str> {
    match entry.split_once('#') {
        Some((file, _)) => file.contains(['.', '/']).then_some(file),
        None => Some(entry),
    }
}

/// The directory of the file of `entry`, `.` for the root or no file.
fn folder_of(entry: &str) -> String {
    Path::new(entry_file(entry).unwrap_or_default())
        .parent()
        .map(|p| p.display().to_string())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| ".".to_string())
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
