//! The figures of the audit page, independent of how they are shown (`specs/audit-dashboard.md`):
//! the overall scores of the debt report, the coverage of the entry points, the stale share, the
//! cost of the last `generate`, the progress of the bounded contexts, and the debt items counted by
//! domain and kind. Read at each call; a figure the run lacks is `None`.

use retrodoc_pipeline::usage_log::{load_history, RunUsage};
use retrodoc_pipeline::{
    build_report, recorded_progress, BehaviourCoverage, DomainMap, RecordedProgress,
};

use crate::debt::{debt, DebtKind, DebtList};
use crate::read_model::GeneratedDocs;

/// The highest level of a heatmap cell.
pub const MAX_LEVEL: u8 = 4;

/// `count` out of `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Share {
    pub count: usize,
    pub total: usize,
}

impl Share {
    /// Rounded to the unit, 0 of nothing.
    #[must_use]
    pub fn percent(self) -> usize {
        (self.count * 100 + self.total / 2)
            .checked_div(self.total)
            .unwrap_or(0)
    }
}

/// The entry points the docs reach; at least one of the two is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// A domain of the run and its cells, in the order of [`DebtKind::ALL`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatRow {
    pub domain: String,
    pub cells: Vec<HeatCell>,
}

/// The figures of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Audit {
    pub confidence: Option<f32>,
    pub business_language: Option<f32>,
    pub coverage: Option<EntryCoverage>,
    /// Stale features and use cases, out of those of the run.
    pub stale: Share,
    /// The last `generate` of the usage history.
    pub cost: Option<RunUsage>,
    pub contexts: Option<RecordedProgress>,
    /// One row per domain, in the order of the run.
    pub heatmap: Vec<HeatRow>,
}

/// The audit of `docs`; the files of the repository it was loaded from are read now.
#[must_use]
pub fn audit(docs: &GeneratedDocs) -> Audit {
    let report = build_report(&DomainMap::default(), docs.features(), docs.use_cases());
    let debt = debt(docs);
    let root = docs.repo_root();
    Audit {
        confidence: report.overall,
        business_language: report.overall_business,
        coverage: root
            .and_then(BehaviourCoverage::load)
            .and_then(|c| entry_coverage(&c)),
        stale: Share {
            count: debt
                .items
                .iter()
                .filter(|i| i.kind == DebtKind::Stale)
                .count(),
            total: docs.features().len() + docs.use_cases().len(),
        },
        cost: root.and_then(|root| {
            load_history(root)
                .into_iter()
                .rev()
                .find(|run| run.command == "generate")
        }),
        contexts: root.and_then(recorded_progress),
        heatmap: heatmap(docs, &debt),
    }
}

fn entry_coverage(coverage: &BehaviourCoverage) -> Option<EntryCoverage> {
    let share = |pairs: Vec<(usize, usize)>| {
        (!pairs.is_empty()).then(|| Share {
            count: pairs.iter().map(|(count, _)| count).sum(),
            total: pairs.iter().map(|(_, total)| total).sum(),
        })
    };
    let placed = share(
        coverage
            .by_domain
            .values()
            .map(|c| (c.entry_points.saturating_sub(c.unplaced), c.entry_points))
            .collect(),
    );
    let cited = share(
        coverage
            .by_feature
            .values()
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

fn heatmap(docs: &GeneratedDocs, debt: &DebtList) -> Vec<HeatRow> {
    let counts: Vec<(String, Vec<usize>)> = docs
        .domains()
        .iter()
        .map(|domain| {
            let of = |kind: DebtKind| {
                debt.items
                    .iter()
                    .filter(|i| i.kind == kind && i.section.domain.as_deref() == Some(&domain.slug))
                    .count()
            };
            (domain.slug.clone(), DebtKind::ALL.map(of).to_vec())
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
            cells: DebtKind::ALL
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
