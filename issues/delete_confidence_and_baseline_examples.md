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
* `issues/delete_one_off_measuring_examples.md` (the pick and how the other deletions were done).
* `docs/dev/commands.md`, `docs/crates/bench.md`, ADR 0026, ADR 0042.

# Hints
* `[[example]]` entries exist only for examples with `test = true`; the others are found by Cargo alone.
