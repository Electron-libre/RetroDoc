//! Quality benchmark (see `issues/done/quality_benchmark.md`): measures the generated functional docs
//! against a hand-written reference instead of only checking that `generate` ends without a warning.
//!
//! - [`reference`]: the hand-written reference of one repository (`benchmark/<repo>/reference.yaml`).
//! - [`matching`]: generated vs reference names (hand-written `matches.yaml` first), recall and precision.
//! - [`metrics`]: the figures read from the artifacts of a run (`.retrodoc/cache/`), with no LLM call.

pub mod evidence;
pub mod frame;
pub mod frozen;
pub mod judge;
pub mod matching;
pub mod metrics;
pub mod questions;
pub mod reference;
pub mod replay;
pub mod replay_judgement;
pub mod replay_rules;
pub mod replay_use_cases;
pub mod summary;
pub mod table;

pub use frame::{tree_evidence, Frame};
pub use frozen::{FrozenInputs, Manifest, ReplayPass};
pub use judge::{judge, second_calls, Judgement, NarrativeRating, SecondCall, Unaudited, Unjudged};
pub use matching::{compare, Comparison, Matches, Pair, Rejected, Score, SCORING_VERSION};
pub use metrics::{Cost, RunMetrics};
pub use questions::{check_questions, Question, QuestionKind, QuestionSet};
pub use reference::{
    Checked, Evidence, EvidenceKind, Reference, ReferenceDomain, ReferenceFeature,
};
pub use replay::{replay_domains, replay_features, replay_table, RunFigures, Scoring};
pub use replay_judgement::{
    parse_labels, replay_judgement, score_labels, JudgementFigures, LabelScore, GOOD, KEPT,
};
pub use replay_rules::{replay_rules, rule_table, RuleFigures};
pub use replay_use_cases::{replay_use_cases, use_case_table, UseCaseFigures};
pub use summary::summary;
pub use table::{load_series, table, RunReport, Series};
