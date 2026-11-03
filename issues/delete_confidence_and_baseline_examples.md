---
priority: low
depends_on: [delete_one_off_measuring_examples]
why: 'a comfort of maintenance split from delete_one_off_measuring_examples for its size; nothing user-facing waits on it'
---
# Delete the confidence and baseline measuring examples

# Goal
Three one-off examples of `crates/retrodoc-bench/examples/` are left by `delete_one_off_measuring_examples`, cut out
of it for the size limit of one issue: `narrative_check_measure`, `confidence_agreement` and `behaviour_baseline`.
They served closed issues and no planned measure of the ledger names them. Expected: they are deleted with every
mention of them, their figures staying in the closed issues and the ledger of measures.

# Findings (audit of 2026-11-03, `delete_one_off_measuring_examples`)
* `narrative_check_measure` (~150 lines): named only by `issues/done/narrative_checked_against_cited_code.md` and past
  measures of the ledger.
* `confidence_agreement` (~140 lines): named by ADR 0026, `docs/crates/pipeline.md` and the doc comment of
  `crates/retrodoc-pipeline/src/confidence/mod.rs` (around line 146).
* `behaviour_baseline` (~210 lines): has an `[[example]]` entry in `crates/retrodoc-bench/Cargo.toml`, a line in
  `docs/dev/commands.md`, and the `//!` doc of the kept example `parsed_slices.rs` compares itself with it.
* About 520 deleted lines in all: one deliverable.

# Approach
1. Delete the three examples with their `[[example]]` entry, their lines in `docs/dev/commands.md` and
   `docs/crates/pipeline.md`, and reword the doc comments that name them (`confidence/mod.rs`, `parsed_slices.rs`).
2. Add a note to ADR 0026 that the example is gone, its figures kept in the closed issue.

# Milestone
Release and code hygiene

# Resources
* `issues/done/delete_one_off_measuring_examples.md` (the pick and how the other deletions were done).
* `docs/dev/commands.md`, `docs/crates/bench.md`, ADR 0026, ADR 0042.

# Hints
* `[[example]]` entries exist only for examples with `test = true`; the others are found by Cargo alone.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
Delete `narrative_check_measure`, `confidence_agreement` and `behaviour_baseline` with every live mention (Cargo
entry, `docs/dev/commands.md`, `docs/crates/pipeline.md`, doc comments of `confidence/mod.rs` and `parsed_slices.rs`),
plus a dated note in ADR 0026. Closed issues and the ledger keep their mentions. No export to drop: `mechanical_score`
is still used by the use-cases replay and `StepTally` is its return type.

## Plan
1. [x] Delete the three examples and reword their mentions; ADR 0026 keeps the example name with a dated note
   pointing to `issues/done/deterministic_confidence_checks.md`; verified by `just check` and a grep finding no
   mention outside closed issues and this one. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 7b2b654

## Summary

### Deliverable 1
- done: the three examples deleted with the `[[example]]` entry of `behaviour_baseline` and its `docs/dev/commands.md`
  line; `docs/crates/pipeline.md` and the doc comment of `StepTally` point to the use-cases replay, the remaining user
  of `mechanical_score`; `parsed_slices.rs` no longer compares itself with `behaviour_baseline`; ADR 0026 keeps the
  example name with a dated note pointing to `issues/done/deterministic_confidence_checks.md`.
- checks: `just check` passed; `deliverable-size` 514 lines; no mention left outside closed issues and this one;
  update-docs: `docs/crates/bench.md` and the map name no example individually (n/a).
- red: none (deletions, no test).
- Code review: no findings
