//! The trends of the audit page (`specs/audit-dashboard.md`, AUD-11 to AUD-15): the figures of
//! each run the history saved, newest first, each marked when it is worse than in the run just
//! before it as shown.

use retrodoc_mcp::audit::Share;
use retrodoc_mcp::debt::DebtKind;
use retrodoc_mcp::history::{SavedRun, Snapshot};

use crate::history::{label, short_commit};
use crate::texts::WebTexts;

pub struct TrendsView {
    /// Newest first.
    pub runs: Vec<TrendRow>,
    /// The names of the kinds of the debt table.
    pub kinds: Vec<&'static str>,
}

pub struct TrendRow {
    /// When it ended, as the history page writes it.
    pub finished: String,
    /// The first 7 characters, `None` with no commit.
    pub commit: Option<String>,
    /// The confidence, the business language, the two shares of the coverage, the debt items.
    pub figures: Vec<TrendCell>,
    /// One per kind of [`TrendsView::kinds`].
    pub debt: Vec<TrendCell>,
}

/// A figure as shown, `None` when the run lacks it.
pub struct TrendCell {
    pub text: Option<String>,
    pub worse: bool,
}

/// A figure of a run before it is shown.
#[derive(Clone, Copy)]
struct Figure {
    value: Option<usize>,
    /// A count of debt: more is worse.
    lower_is_better: bool,
    unit: &'static str,
}

impl Figure {
    fn score(value: Option<usize>) -> Self {
        Self {
            value,
            lower_is_better: false,
            unit: "%",
        }
    }

    fn debt(count: usize) -> Self {
        Self {
            value: Some(count),
            lower_is_better: true,
            unit: "",
        }
    }

    /// `self` as shown, marked when it is worse than `then`, a figure of the run before.
    fn cell(self, then: Option<Self>) -> TrendCell {
        let worse = match (self.value, then.and_then(|t| t.value)) {
            (Some(now), Some(then)) if self.lower_is_better => now > then,
            (Some(now), Some(then)) => now < then,
            _ => false,
        };
        TrendCell {
            text: self.value.map(|v| format!("{v}{}", self.unit)),
            worse,
        }
    }
}

/// The trends of `runs` (oldest first, as `history::load` gives them), `None` with fewer than two.
#[must_use]
pub fn trends(runs: &[SavedRun], texts: &WebTexts) -> Option<TrendsView> {
    if runs.len() < 2 {
        return None;
    }
    let kinds: Vec<DebtKind> = DebtKind::ALL
        .into_iter()
        .filter(|kind| runs.iter().any(|run| debt_of(&run.snapshot, *kind) > 0))
        .collect();
    let figures: Vec<(Vec<Figure>, Vec<Figure>)> = runs
        .iter()
        .map(|run| {
            let debt = kinds
                .iter()
                .map(|kind| Figure::debt(debt_of(&run.snapshot, *kind)));
            (overall(&run.snapshot), debt.collect())
        })
        .collect();
    let cells = |now: &[Figure], then: Option<&[Figure]>| -> Vec<TrendCell> {
        now.iter()
            .enumerate()
            .map(|(at, figure)| figure.cell(then.map(|then| then[at])))
            .collect()
    };
    let rows = runs
        .iter()
        .zip(&figures)
        .enumerate()
        .map(|(at, (run, now))| {
            let then = at.checked_sub(1).map(|before| &figures[before]);
            TrendRow {
                finished: label(run),
                commit: short_commit(&run.snapshot),
                figures: cells(&now.0, then.map(|t| t.0.as_slice())),
                debt: cells(&now.1, then.map(|t| t.1.as_slice())),
            }
        });
    Some(TrendsView {
        runs: rows.rev().collect(),
        kinds: kinds.iter().map(|kind| texts.debt_kind(*kind)).collect(),
    })
}

/// The confidence and the business language in rounded percents, the two shares of the coverage
/// as the audit page rounds them, the debt items.
fn overall(snapshot: &Snapshot) -> Vec<Figure> {
    let figures = &snapshot.figures;
    let coverage = figures.coverage.as_ref();
    vec![
        Figure::score(figures.confidence.map(percent)),
        Figure::score(figures.business_language.map(percent)),
        Figure::score(coverage.and_then(|c| c.placed).map(Share::percent)),
        Figure::score(coverage.and_then(|c| c.cited).map(Share::percent)),
        Figure::debt(figures.debt.iter().map(|d| d.count).sum()),
    ]
}

/// `0.804` → `80`, as the badges of the pages round a score.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn percent(score: f32) -> usize {
    (score * 100.0).round().max(0.0) as usize
}

fn debt_of(snapshot: &Snapshot, kind: DebtKind) -> usize {
    snapshot
        .figures
        .debt
        .iter()
        .filter(|d| d.kind == kind.key())
        .map(|d| d.count)
        .sum()
}
