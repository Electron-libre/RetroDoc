//! The trends of the audit page (`specs/audit-dashboard.md`, AUD-11 to AUD-15): the figures of
//! each run the history saved, newest first, each marked when it is worse than in the run just
//! before it as shown, and a line chart per score drawn on the server as SVG (AUD-14).

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
    /// One per score some run has, in the order of the columns of the table.
    pub charts: Vec<ChartView>,
}

/// The width and height of the box of a chart, and where its plot sits in it: the runs evenly
/// spaced from `LEFT` to `RIGHT`, 0% at `BOTTOM`, 100% at `TOP`; the room to the right holds the
/// value of the newest run.
const WIDTH: f32 = 240.0;
const HEIGHT: f32 = 100.0;
const LEFT: f32 = 34.0;
const RIGHT: f32 = 200.0;
const TOP: f32 = 10.0;
const BOTTOM: f32 = 90.0;

/// A score over the runs, oldest left.
pub struct ChartView {
    pub name: &'static str,
    /// The heights of the gridlines and their labels: 0%, 50%, 100%.
    pub grid: Vec<(f32, &'static str)>,
    /// One per run that has the score.
    pub points: Vec<ChartPoint>,
    /// Between two runs side by side that both have it.
    pub segments: Vec<Segment>,
    pub width: f32,
    pub height: f32,
    pub left: f32,
    pub right: f32,
}

pub struct Segment {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

pub struct ChartPoint {
    pub x: f32,
    pub y: f32,
    /// The value, `80%`.
    pub value: String,
    /// When its run ended and the value, its tooltip.
    pub title: String,
    /// The newest point, labelled with its value.
    pub newest: bool,
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
    /// The name of its column.
    name: &'static str,
    value: Option<usize>,
    /// A count of debt: more is worse.
    lower_is_better: bool,
    unit: &'static str,
}

impl Figure {
    fn score(name: &'static str, value: Option<usize>) -> Self {
        Self {
            name,
            value,
            lower_is_better: false,
            unit: "%",
        }
    }

    fn debt(name: &'static str, count: usize) -> Self {
        Self {
            name,
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
                .map(|kind| Figure::debt(texts.debt_kind(*kind), debt_of(&run.snapshot, *kind)));
            (overall(&run.snapshot, texts), debt.collect())
        })
        .collect();
    let cells = |now: &[Figure], then: Option<&[Figure]>| -> Vec<TrendCell> {
        now.iter()
            .enumerate()
            .map(|(at, figure)| figure.cell(then.map(|then| then[at])))
            .collect()
    };
    let labels: Vec<String> = runs.iter().map(label).collect();
    let rows =
        labels
            .iter()
            .zip(runs)
            .zip(&figures)
            .enumerate()
            .map(|(at, ((label, run), now))| {
                let then = at.checked_sub(1).map(|before| &figures[before]);
                TrendRow {
                    finished: label.clone(),
                    commit: short_commit(&run.snapshot),
                    figures: cells(&now.0, then.map(|t| t.0.as_slice())),
                    debt: cells(&now.1, then.map(|t| t.1.as_slice())),
                }
            });
    // The scores, read off the oldest run: every run has the same figures in the same order.
    let charts = figures[0]
        .0
        .iter()
        .enumerate()
        .filter(|(_, figure)| !figure.lower_is_better)
        .filter_map(|(at, figure)| {
            let values: Vec<Option<usize>> = figures.iter().map(|f| f.0[at].value).collect();
            chart(figure.name, &values, &labels)
        })
        .collect();
    Some(TrendsView {
        charts,
        runs: rows.rev().collect(),
        kinds: kinds.iter().map(|kind| texts.debt_kind(*kind)).collect(),
    })
}

/// The chart of the percents `values` of the runs that ended at `labels`, `None` when no run has
/// one.
#[allow(clippy::cast_precision_loss)]
fn chart(name: &'static str, values: &[Option<usize>], labels: &[String]) -> Option<ChartView> {
    let step = (RIGHT - LEFT) / (values.len().max(2) - 1) as f32;
    let y = |percent: usize| BOTTOM - (BOTTOM - TOP) * percent as f32 / 100.0;
    let newest = values.iter().rposition(Option::is_some)?;
    let at: Vec<Option<ChartPoint>> = values
        .iter()
        .zip(labels)
        .enumerate()
        .map(|(run, (value, label))| {
            value.map(|value| ChartPoint {
                x: LEFT + step * run as f32,
                y: y(value),
                value: format!("{value}%"),
                title: format!("{label}: {value}%"),
                newest: run == newest,
            })
        })
        .collect();
    Some(ChartView {
        name,
        grid: [(0, "0%"), (50, "50%"), (100, "100%")]
            .map(|(percent, text)| (y(percent), text))
            .to_vec(),
        segments: at
            .windows(2)
            .filter_map(|pair| {
                let (from, to) = (pair[0].as_ref()?, pair[1].as_ref()?);
                Some(Segment {
                    x1: from.x,
                    y1: from.y,
                    x2: to.x,
                    y2: to.y,
                })
            })
            .collect(),
        points: at.into_iter().flatten().collect(),
        width: WIDTH,
        height: HEIGHT,
        left: LEFT,
        right: RIGHT,
    })
}

/// The confidence and the business language in rounded percents, the two shares of the coverage
/// as the audit page rounds them, the debt items.
fn overall(snapshot: &Snapshot, texts: &WebTexts) -> Vec<Figure> {
    let figures = &snapshot.figures;
    let coverage = figures.coverage.as_ref();
    let [_, _, confidence, business, placed, cited, debt] = texts.audit_page.trend_columns;
    vec![
        Figure::score(confidence, figures.confidence.map(percent)),
        Figure::score(business, figures.business_language.map(percent)),
        Figure::score(placed, coverage.and_then(|c| c.placed).map(Share::percent)),
        Figure::score(cited, coverage.and_then(|c| c.cited).map(Share::percent)),
        Figure::debt(debt, figures.debt.iter().map(|d| d.count).sum()),
    ]
}

/// `0.804` → `80`, as the badges of the pages round a score: a tie to even, as `{:.0}` does.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn percent(score: f32) -> usize {
    (score * 100.0).round_ties_even().max(0.0) as usize
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

#[cfg(test)]
mod tests {
    use super::*;

    // spec: AUD-11 (internal: a score on a tie of the rounding, which no fixture of the page hits)
    #[test]
    fn a_score_rounds_as_its_badge_does() {
        for score in [0.625_f32, 0.125, 0.805, 0.804] {
            assert_eq!(
                format!("{}%", percent(score)),
                format!("{:.0}%", score * 100.0),
                "{score}"
            );
        }
    }
}
