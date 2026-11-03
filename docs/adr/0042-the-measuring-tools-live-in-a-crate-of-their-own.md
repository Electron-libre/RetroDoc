# 0042. The measuring tools live in a crate of their own

Status: Proposed

## Context

`retrodoc-pipeline` held the passes and also the benchmark (`src/benchmark/`: scoring, judge, replays of one pass on
frozen inputs, about 5.6k lines) and 22 measuring examples (about 5.4k lines): a quarter of the crate was measuring
tooling, shipped with the product code and growing with each issue of the quality milestone. The replays reach
items of the passes that are private to the crate (the rules judgement, the JSON answer reader, the text and naming
helpers, the artifact files) and the unit tests of the benchmark use the fake LLM of the crate's `testing` module.

## Decision

A crate `retrodoc-bench` holds `benchmark/`, the measuring examples and their integration test; it depends on
`retrodoc-pipeline`, and `retrodoc-cli` calls it for `benchmark` and `benchmark-table`. The pass items the
replays reach stay private in the default build of `retrodoc-pipeline`: a cargo feature `bench` exposes them
through a `#[doc(hidden)]` module `bench_support`, and a feature `testing` exposes the `testing` module (fake LLM,
log capture) to the unit tests of `retrodoc-bench`. The files are moved unchanged: the root of `retrodoc-bench`
brings the modules of `retrodoc-pipeline` into scope under their own names, so the `crate::<module>` paths of the
moved files still resolve.

Rejected: keeping the crate and only moving the examples to a `measures/` directory (the benchmark, the largest
part, stays in the product crate); making the reached items `pub` in the default build (the public surface of the
pipeline grows with every replay).

## Consequences

The passes crate shrinks back to the passes; a change of the benchmark no longer rebuilds or retests the pipeline.
The examples run with `-p retrodoc-bench`: the commands of `docs/dev/commands.md`, the skills and the reuse rule of
the ledger of measures name the new paths. The `crate::<module>` aliases of the root of `retrodoc-bench` are a
convenience of the move: a file edited later may import from `retrodoc_pipeline` directly.
