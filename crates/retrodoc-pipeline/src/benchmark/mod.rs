//! Quality benchmark (see `issues/quality_benchmark.md`): measures the generated functional docs
//! against a hand-written reference instead of only checking that `generate` ends without a warning.
//!
//! - [`reference`]: the hand-written reference of one repository (`benchmark/<repo>/reference.yaml`).
//! - [`metrics`]: the figures read from the artifacts of a run (`.retrodoc/cache/`), with no LLM call.

pub mod metrics;
pub mod reference;

pub use metrics::{Cost, RunMetrics};
pub use reference::{Reference, ReferenceDomain, ReferenceFeature};
