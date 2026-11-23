//! The measuring tools of `retrodoc`, kept out of the passes (ADR 0042): the quality [`benchmark`] (scoring,
//! judge, replays of one pass on frozen inputs), the [`pass_ratios`] of the saved runs, the measuring examples of
//! `examples/` and their test.
//!
//! The modules of `retrodoc-pipeline` are in scope at the root under their own names, so the files moved from
//! it keep their `crate::<module>` paths. Those of `bench_support` hold only what the moved files reach, not the
//! whole module of the same name: a file edited later may import from `retrodoc_pipeline` directly.

pub mod benchmark;
pub mod pass_ratios;

use retrodoc_core::bm25;
use retrodoc_pipeline::bench_support::{artifact, domains, naming, response, rules, text, walked};
#[cfg(test)]
use retrodoc_pipeline::testing;
use retrodoc_pipeline::{
    actors, brief, confidence, entry_points, error, features, slices, sources, surface, use_cases,
    vocabulary,
};
