//! The figures of the audit page, independent of how they are shown (`specs/audit-dashboard.md`):
//! the overall scores of the debt report, the coverage of the entry points, the stale share, the
//! cost of the last `generate`, the progress of the bounded contexts, the sections a human
//! reviewed, and the debt items counted by domain and kind. Read at each call; a figure the run lacks is `None`.

use std::collections::BTreeMap;

use retrodoc_pipeline::usage_log::load_history;
use retrodoc_pipeline::{recorded_progress, BehaviourCoverage};
use serde::{Deserialize, Serialize};
// What the figures are made of and how the recap of a run writes them, for whoever shows them.
pub use retrodoc_pipeline::usage_log::{thousands, ModelRecord, PassRecord, RunUsage};
pub use retrodoc_pipeline::{format_duration, RecordedProgress};

use crate::debt::{debt_of, docs_report, DebtKind, DebtList};
use crate::read_model::GeneratedDocs;
use crate::reviews::{Reviews, SectionKind, Status};

/// The highest level of a heatmap cell.
pub const MAX_LEVEL: u8 = 4;

/// `count` out of `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    pub count: usize,
    pub total: usize,
}

impl Share {
    /// Rounded to the unit, 0 of nothing; never 0 when some are counted nor 100 when some are
    /// left out, so a share that rounds away still shows.
    #[must_use]
    pub fn percent(self) -> usize {
        let rounded = (self.count * 100 + self.total / 2)
            .checked_div(self.total)
            .unwrap_or(0);
        if self.count == 0 || self.count >= self.total {
            rounded
        } else {
            rounded.clamp(1, 99)
        }
    }
}

/// The entry points the docs reach; at least one of the two is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryCoverage {
    /// Held by a feature, out of those of the bounded contexts.
    pub placed: Option<Share>,
    /// Cited by a use case, out of those of the features.
    pub cited: Option<Share>,
}

/// The debt items of one kind in one domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatCell {
    pub kind: DebtKind,
    pub count: usize,
    /// 0 for no item, else `ceil(MAX_LEVEL × count / max)`, `max` the largest count of the table.
    pub level: u8,
}

/// A domain of the run and its cells, in the order of [`DebtKind::WITH_DOMAIN`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatRow {
    pub domain: String,
    pub cells: Vec<HeatCell>,
}

/// The sections of one kind a human reviewed, out of those of the run, by status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reviewed {
    pub kind: SectionKind,
    pub share: Share,
    pub validated: usize,
    pub wrong: usize,
    pub to_review: usize,
}

/// The figures of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Audit {
    pub confidence: Option<f32>,
    pub business_language: Option<f32>,
    pub coverage: Option<EntryCoverage>,
    /// Stale features and use cases, out of the features of the run and their use cases.
    pub stale: Share,
    /// The last `generate` of the usage history.
    pub cost: Option<RunUsage>,
    pub contexts: Option<RecordedProgress>,
    /// The domains, then the features, then the use cases.
    pub reviewed: [Reviewed; 3],
    /// One row per domain, in the order of the run.
    pub heatmap: Vec<HeatRow>,
}

/// The audit of `docs` with its `reviews`; the files of the repository it was loaded from are
/// read now.
#[must_use]
pub fn audit(docs: &GeneratedDocs, reviews: &Reviews) -> Audit {
    let report = docs_report(docs);
    let (confidence, business_language) = (report.overall, report.overall_business);
    let debt = debt_of(docs, report, reviews);
    let root = docs.repo_root();
    // The stale check reaches a use case through its feature only.
    let sections = docs.features().len()
        + docs
            .features()
            .iter()
            .map(|f| docs.use_cases_of(&f.slug).count())
            .sum::<usize>();
    Audit {
        confidence,
        business_language,
        coverage: root
            .and_then(BehaviourCoverage::load)
            .and_then(|c| entry_coverage(docs, &c)),
        stale: Share {
            count: debt
                .items
                .iter()
                .filter(|i| i.kind == DebtKind::Stale)
                .count(),
            total: sections,
        },
        cost: root.and_then(|root| {
            load_history(root)
                .into_iter()
                .rev()
                .find(|run| run.command == "generate")
        }),
        contexts: root.and_then(recorded_progress),
        reviewed: reviewed(docs, reviews),
        heatmap: heatmap(docs, &debt),
    }
}

/// The coverage of the domains and features of the run: the saved counts of the others are those
/// of an older run.
pub(crate) fn entry_coverage(
    docs: &GeneratedDocs,
    coverage: &BehaviourCoverage,
) -> Option<EntryCoverage> {
    let share = |pairs: Vec<(usize, usize)>| {
        (!pairs.is_empty()).then(|| Share {
            count: pairs.iter().map(|(count, _)| count).sum(),
            total: pairs.iter().map(|(_, total)| total).sum(),
        })
    };
    let placed = share(
        coverage
            .by_domain
            .iter()
            .filter(|(slug, _)| docs.domains().iter().any(|d| &d.slug == *slug))
            .map(|(_, c)| c)
            .map(|c| (c.entry_points.saturating_sub(c.unplaced), c.entry_points))
            .collect(),
    );
    let cited = share(
        coverage
            .by_feature
            .iter()
            .filter(|(slug, _)| docs.features().iter().any(|f| &f.slug == *slug))
            .map(|(_, c)| c)
            .map(|c| {
                (
                    c.entry_points.saturating_sub(c.uncovered.len()),
                    c.entry_points,
                )
            })
            .collect(),
    );
    (placed.is_some() || cited.is_some()).then_some(EntryCoverage { placed, cited })
}

/// The reviews of the domains, the features and the use cases the run holds, a use case reached
/// through its feature as the stale share counts it.
fn reviewed(docs: &GeneratedDocs, reviews: &Reviews) -> [Reviewed; 3] {
    let features = docs.features();
    let ids = |kind: SectionKind| -> Vec<String> {
        match kind {
            SectionKind::Domain => docs.domains().iter().map(|d| d.slug.clone()).collect(),
            SectionKind::Feature => features.iter().map(|f| f.slug.clone()).collect(),
            SectionKind::UseCase => features
                .iter()
                .flat_map(|f| {
                    docs.use_cases_of(&f.slug)
                        .map(move |u| format!("{}/{}", f.slug, u.slug))
                })
                .collect(),
        }
    };
    [
        SectionKind::Domain,
        SectionKind::Feature,
        SectionKind::UseCase,
    ]
    .map(|kind| {
        let ids = ids(kind);
        let statuses: Vec<Status> = reviews
            .reviews()
            .iter()
            .filter(|r| r.section.kind == kind && ids.contains(&r.section.id))
            .map(|r| r.status)
            .collect();
        let count = |status: Status| statuses.iter().filter(|s| **s == status).count();
        Reviewed {
            kind,
            share: Share {
                count: statuses.len(),
                total: ids.len(),
            },
            validated: count(Status::Validated),
            wrong: count(Status::Wrong),
            to_review: count(Status::ToReview),
        }
    })
}

fn heatmap(docs: &GeneratedDocs, debt: &DebtList) -> Vec<HeatRow> {
    let mut by_cell: BTreeMap<(&str, usize), usize> = BTreeMap::new();
    for item in &debt.items {
        let kind = DebtKind::WITH_DOMAIN.iter().position(|k| *k == item.kind);
        if let (Some(domain), Some(kind)) = (item.domain(), kind) {
            *by_cell.entry((domain, kind)).or_default() += 1;
        }
    }
    let counts: Vec<(String, Vec<usize>)> = docs
        .domains()
        .iter()
        .map(|domain| {
            let row = (0..DebtKind::WITH_DOMAIN.len())
                .map(|kind| {
                    by_cell
                        .get(&(domain.slug.as_str(), kind))
                        .copied()
                        .unwrap_or(0)
                })
                .collect();
            (domain.slug.clone(), row)
        })
        .collect();
    let max = counts
        .iter()
        .flat_map(|(_, row)| row)
        .copied()
        .max()
        .unwrap_or(0);
    counts
        .into_iter()
        .map(|(domain, row)| HeatRow {
            domain,
            cells: DebtKind::WITH_DOMAIN
                .iter()
                .zip(row)
                .map(|(kind, count)| HeatCell {
                    kind: *kind,
                    count,
                    level: level(count, max),
                })
                .collect(),
        })
        .collect()
}

/// `ceil(MAX_LEVEL × count / max)`, 0 for no item.
fn level(count: usize, max: usize) -> u8 {
    if count == 0 || max == 0 {
        return 0;
    }
    let level = (count * usize::from(MAX_LEVEL)).div_ceil(max);
    u8::try_from(level).unwrap_or(MAX_LEVEL).min(MAX_LEVEL)
}

#[cfg(test)]
mod tests;
