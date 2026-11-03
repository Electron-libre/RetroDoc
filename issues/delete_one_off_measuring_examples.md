---
priority: low
depends_on: [measure_tooling_out_of_the_pipeline_crate]
why: 'a comfort of maintenance: the examples cost only build and test time, nothing user-facing waits on it'
---
# Delete the one-off measuring examples and fix the usage line of the kept ones

# Goal
`crates/retrodoc-bench/examples/` holds 22 programs; most served one closed issue and nothing names them now, yet
they are built and tested with the workspace. Expected: the examples the user picks are deleted (their figures stay in
the closed issues and the ledger of measures), and the `//!` usage line of each kept example says
`cargo run -q -p retrodoc-bench --example <name>` instead of `-p retrodoc-pipeline`.

# Findings (audit of 2026-11-03, `measure_tooling_out_of_the_pipeline_crate`)
* The move to `retrodoc-bench` left the examples unchanged: their usage lines still name `-p retrodoc-pipeline`,
  which no longer has examples (editing them in the move would have counted each moved file twice).
* Named nowhere among the skills, `docs/dev/commands.md`, the ledger's measures or an open issue:
  `confidence_agreement` (ADR 0026 only), `orphan_placement_measure`, `read_overlap` (ADR 0028 only).
* Named only by `docs/dev/commands.md`, one closed issue each, none in the planned measures of the ledger:
  `behaviour_baseline`, `features_by_domain_measure`, `public_api_reach_measure`, `route_owner_measure`,
  `rules_answer_size_measure`, `rules_noise_measure`, `technical_usecases_measure`, `use_cases_answer_size_measure`,
  `parsed_slices`, `context_map_measure`, `routing_duplicates_measure`, `form_pairs_measure`, `code_read_overlap`,
  `narrative_check_measure`.
* Tools still in use: `pass_replay`, `freeze_inputs`, `entry_points_replay`, `rules_judge_replay`, `placement_replay`.
* The 17 candidates hold about 3.4k lines: deleting them all is over the size limit of one issue.

# Approach
1. The user picks the examples to delete from the candidates (a closed issue or an ADR that names one keeps its
   figures; the ADR may get a note that the example is gone).
2. Delete them with their `[[example]]` entry in `crates/retrodoc-bench/Cargo.toml`, their line in
   `docs/dev/commands.md` and in `docs/crates/pipeline.md`; a deliverable under ~1,000 deleted lines each, the issue
   split when the pick is over ~3,000.
3. Fix the usage line of the examples kept.

# Milestone
Release and code hygiene

# Resources
* `docs/dev/commands.md` (the examples and what they measure), `docs/crates/bench.md`, ADR 0042.
* The ledger of measures (outside git, see `CLAUDE.md`): the measures that name an example.
* Skills `quality-benchmark` and `smoke-test` (the examples they run).

# Hints
* `crates/retrodoc-bench/tests/benchmark_files.rs` and `parsed_slices.rs` refer to other examples
  (`entry_points_replay`, `behaviour_baseline`): check what a deletion breaks.
