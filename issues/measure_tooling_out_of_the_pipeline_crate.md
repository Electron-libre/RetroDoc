---
priority: low
why: 'a comfort of maintenance: nothing user-facing waits on it; easier once the quality milestone stops moving the benchmark code'
depends_on: [judge_overstates_feature_precision]
---
# Move the measuring tools out of the pipeline crate

# Goal
`retrodoc-pipeline` holds the passes and also the benchmark (scoring, judge, replays) and 22 measuring examples: about
a quarter of the crate is measuring tooling that ships with the product code and grows with every issue. Expected:
the passes stay in `retrodoc-pipeline`, the benchmark and the measures sit in a crate of their own (or a clearly
separated place), with no change of behaviour of `generate` or `retrodoc benchmark`.

# Findings (audit of 2026-11-03)
* Workspace: about 74k lines of Rust; `retrodoc-pipeline` 44k in 129 files.
* `crates/retrodoc-pipeline/src/benchmark/`: 13 files, about 5.4k lines (`judge.rs` 1,030).
* `crates/retrodoc-pipeline/examples/`: 22 programs (`pass_replay`, `freeze_inputs`, `*_measure`, `*_replay`), about
  5.4k lines; several served one issue each and may be dead now.

# Approach
1. Measure first: which examples are still named by a skill, `docs/dev/commands.md`, the ledger's reusable measures
   or an open issue; list the others as candidates to delete (the user decides).
2. Candidates (can be combined):
   (a) a `retrodoc-bench` crate with `benchmark/` and the kept examples, depending on `retrodoc-pipeline`;
   `retrodoc-cli` calls it for `benchmark` and `benchmark-table`;
   (b) delete the one-off measures nothing names, their figures stay in the closed issues and the ledger;
   (c) keep the crate as is, and only move the examples to `crates/retrodoc-pipeline/measures/` with a README.
3. Move in a deliverable of its own (a moved and edited file counts twice in the size).

# Milestone
Release and code hygiene

# Resources
* `docs/crates/pipeline.md`, `docs/ARCHITECTURE.md` (crate graph), `CLAUDE.md` (architecture), `docs/dev/commands.md`.
* Skills `quality-benchmark` and `smoke-test` (the commands they run).
* The ledger of measures (outside git, see `CLAUDE.md`): the measures that name an example.

# Hints
* The replays reach private items of the passes: what they need may have to become `pub` or move behind a
  `testing` module; keep the public surface of `retrodoc-pipeline` small.
* The crate graph is drawn in `CLAUDE.md` and `docs/ARCHITECTURE.md`: update both.
