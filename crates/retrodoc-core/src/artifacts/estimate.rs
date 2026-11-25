//! The estimate of a `generate`, as printed before its first LLM call and as saved with its run in the usage
//! history (`specs/run-estimate.md`), and how a measured figure stands against it. The pipeline computes it from
//! the measured ratios (`retrodoc_pipeline::estimate`); the readers of the history read it.

use serde::{Deserialize, Serialize};

/// A low and a high bound.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub low: u64,
    pub high: u64,
}

impl Range {
    /// Adds the bounds of `other` to these.
    pub fn add(&mut self, other: Self) {
        self.low += other.low;
        self.high += other.high;
    }

    /// Whether the bounds allow anything above zero.
    #[must_use]
    pub fn above_zero(self) -> bool {
        self.high > 0
    }

    /// How a measured `figure` missed the range: `None` within it.
    #[must_use]
    pub fn miss(self, figure: u64) -> Option<Miss> {
        let (above, bound) = if figure > self.high {
            (true, self.high)
        } else if figure < self.low {
            (false, self.low)
        } else {
            return None;
        };
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        let percent = (bound > 0)
            .then(|| ((figure as f64 - bound as f64) / bound as f64 * 100.0).round() as i64);
        Some(Miss {
            above,
            bound,
            percent,
        })
    }
}

/// A measured figure out of its estimated range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Miss {
    /// Above the high bound, else below the low one.
    pub above: bool,
    /// The bound missed.
    pub bound: u64,
    /// The miss in percent of the bound, signed; `None` against a bound of zero.
    pub percent: Option<i64>,
}

/// What a pass measured, to set against its estimate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Measured {
    pub calls: u64,
    /// Among `calls`, those that reported no token usage: the token counts are then a lower bound.
    pub calls_without_usage: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// How a measured figure stands against its range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Against {
    Within,
    Missed(Miss),
    /// Below the range, but a token count some calls of the pass left out: a lower bound, no miss.
    LowerBound(Miss),
}

/// One figure of a pass set against its range: its label (`calls`, `tokens in`, `tokens out`), the measured
/// figure and how it stands.
pub type Figure = (&'static str, u64, Against);

/// Why a pass is estimated as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Basis {
    /// Its measured cost per source file, times the files to read.
    PerSourceFile,
    /// Its measured cost per run.
    PerRun,
    /// Its measured cost per unit, times the units the reviews `wrong` not yet answered ask again or
    /// cascade into (`specs/run-estimate.md`, EST-12, EST-13).
    Reviewed,
    /// The run turns it off.
    Off,
    /// The cache saves it: no file changed, or its artifact is saved.
    Cached,
    /// Measured with no call on every run.
    NoCall,
    /// The table has no ratio for it: estimated at zero.
    Unmeasured,
}

/// The estimate of one pass, as printed and as saved with its run in the usage history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassEstimate {
    pub pass: String,
    pub basis: Basis,
    pub calls: Range,
    pub prompt_tokens: Range,
    pub completion_tokens: Range,
}

impl PassEstimate {
    /// Whether the estimate allows a call or a token to this pass.
    #[must_use]
    pub fn is_estimated(&self) -> bool {
        self.calls.above_zero()
            || self.prompt_tokens.above_zero()
            || self.completion_tokens.above_zero()
    }

    /// Whether this pass is set against its estimate: estimated, or it made a call.
    #[must_use]
    pub fn is_compared(&self, measured: Measured) -> bool {
        self.is_estimated() || measured.calls > 0
    }

    /// The calls, prompt tokens and completion tokens `measured` against their ranges.
    #[must_use]
    pub fn against(&self, measured: Measured) -> [Figure; 3] {
        let token = |figure: u64, range: Range| match range.miss(figure) {
            None => Against::Within,
            Some(miss) if !miss.above && measured.calls_without_usage > 0 => {
                Against::LowerBound(miss)
            }
            Some(miss) => Against::Missed(miss),
        };
        [
            (
                "calls",
                measured.calls,
                self.calls
                    .miss(measured.calls)
                    .map_or(Against::Within, Against::Missed),
            ),
            (
                "tokens in",
                measured.prompt_tokens,
                token(measured.prompt_tokens, self.prompt_tokens),
            ),
            (
                "tokens out",
                measured.completion_tokens,
                token(measured.completion_tokens, self.completion_tokens),
            ),
        ]
    }
}

/// The estimate of a run, as printed and as saved with it in the usage history (`RunUsage`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEstimate {
    /// The source files of the run and, on a rerun, those changed (from the `EstimateInputs` of the pipeline).
    pub source_files: usize,
    pub changed_files: Option<usize>,
    pub passes: Vec<PassEstimate>,
    pub calls: Range,
    pub prompt_tokens: Range,
    pub completion_tokens: Range,
}
