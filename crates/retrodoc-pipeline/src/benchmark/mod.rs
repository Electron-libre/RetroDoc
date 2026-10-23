//! Quality benchmark (see `issues/done/quality_benchmark.md`): measures the generated functional docs
//! against a hand-written reference instead of only checking that `generate` ends without a warning.
//!
//! - [`reference`]: the hand-written reference of one repository (`benchmark/<repo>/reference.yaml`).
//! - [`matching`]: generated vs reference names (hand-written `matches.yaml` first), recall and precision.
//! - [`metrics`]: the figures read from the artifacts of a run (`.retrodoc/cache/`), with no LLM call.

pub mod frozen;
pub mod judge;
pub mod matching;
pub mod metrics;
pub mod reference;
pub mod replay;
pub mod summary;
pub mod table;

pub use frozen::{FrozenInputs, Manifest, ReplayPass};
pub use judge::{judge, Judgement, NarrativeRating};
pub use matching::{compare, Comparison, Matches, Pair, Score};
pub use metrics::{Cost, RunMetrics};
pub use reference::{Reference, ReferenceDomain, ReferenceFeature};
pub use replay::{replay_domains, replay_table, RunFigures, Scoring};
pub use summary::summary;
pub use table::{load_series, table, RunReport, Series};
