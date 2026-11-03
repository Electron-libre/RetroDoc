//! The measuring tools of `retrodoc`, kept out of the passes (ADR 0042): the quality [`benchmark`] (scoring,
//! judge, replays of one pass on frozen inputs), the measuring examples of `examples/` and their test.
//!
//! The modules of `retrodoc-pipeline` are in scope at the root under their own names, so the files moved from
//! it keep their `crate::<module>` paths. Those of `bench_support` hold only what the moved files reach, not the
//! whole module of the same name: a file edited later may import from `retrodoc_pipeline` directly.

pub mod benchmark;

use retrodoc_pipeline::bench_support::{
    artifact, domains, naming, progress, response, rules, text, walked,
};
#[cfg(test)]
use retrodoc_pipeline::testing;
use retrodoc_pipeline::{
    actors, bm25, brief, confidence, entry_points, error, features, glossary, slices, sources,
    surface, usage_log, use_cases, vocabulary,
};
