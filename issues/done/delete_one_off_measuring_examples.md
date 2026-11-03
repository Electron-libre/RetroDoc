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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
Delete the one-off examples of `crates/retrodoc-bench/examples/` the user picks, with their `[[example]]` entry and
their lines in `docs/dev/commands.md`, `docs/crates/pipeline.md` and the doc comments that name them (a note in ADRs
0026, 0028, 0029 that the example is gone); fix the usage line of the kept ones. No change to the pipeline's code.

## Plan
1. [x] Usage lines: the `//!` usage line of the kept examples says `-p retrodoc-bench`; verified by `just check` and a
   grep for `-p retrodoc-pipeline --example` (~10 lines). (autonomous)
2. [x] Entry points and contexts: delete `context_map_measure`, `route_owner_measure`, `routing_duplicates_measure`,
   `public_api_reach_measure`, `orphan_placement_measure`; verified by `just check` (~760 lines). (autonomous)
3. [x] Use cases: delete `technical_usecases_measure`, `form_pairs_measure`, `use_cases_answer_size_measure`;
   verified by `just check` (~970 lines). (autonomous)
4. [x] Rules and code reading: delete `rules_answer_size_measure`, `rules_noise_measure`, `code_read_overlap`,
   `read_overlap`, `features_by_domain_measure`; verified by `just check` (~980 lines). (autonomous)
5. [x] Fixes of the `high` review of the whole issue: drop the re-exports of `retrodoc-pipeline` only the deleted
   examples used and the `async-trait` dev-dependency of `retrodoc-bench`; ADRs 0028 and 0029 keep their text with a
   note instead of a rewrite; verified by `just check` and clippy on the default build of the pipeline. (autonomous)

Kept: `pass_replay`, `freeze_inputs`, `entry_points_replay`, `rules_judge_replay`, `placement_replay`, `parsed_slices`
(the only caller of `Follow::Parsed`, `Follow::ParsedThenLexical` and `CodeIndex::slice_with` of ADR 0027: deleting
it leaves them dead).

## Decisions
- Mode: autonomous
- Base commit: f8941084c61889b80ce101236ea6966007c8dc15
- Pick: the 16 candidates but `parsed_slices` (answer 1 a).
- Size: the pick is ~3,230 lines; `narrative_check_measure`, `confidence_agreement` and `behaviour_baseline` go to
  the follow-up issue `delete_confidence_and_baseline_examples` (answer 2 a).

## Summaries

### Deliverable 1
- done: the usage line of the six kept examples and of the three of `delete_confidence_and_baseline_examples` names
  `-p retrodoc-bench`; the examples deleted by the next deliverables are left as they are.
- checks: `just check` passed; the grep finds `-p retrodoc-pipeline --example` only in the examples still to delete.
- red: none (doc comments only, no test).
- Code review: no findings

### Deliverable 2
- done: the five examples deleted with their `[[example]]` entries and `docs/dev/commands.md` lines;
  `docs/crates/pipeline.md` and ADR 0029 point to the closed issue holding the figures instead.
- checks: `just check` passed; `deliverable-size` 755 lines; no mention left outside closed issues and this one.
- red: none (deletions, no test).
- Code review: no findings

### Deliverable 3
- done: the three examples deleted with the `[[example]]` entry of `use_cases_answer_size_measure` and their
  `docs/dev/commands.md` lines; no other document named them.
- checks: `just check` passed; `deliverable-size` 954 lines.
- red: none (deletions, no test).
- Code review: no findings

### Deliverable 4
- done: the five examples deleted with their `docs/dev/commands.md` lines; ADR 0028 points to the closed issue
  holding the figure of `read_overlap`.
- checks: `just check` passed; `deliverable-size` 968 lines.
- red: none (deletions, no test).
- Code review: no findings

### Deliverable 5
- done: the `high` review of `f894108..HEAD` found 4 points, all fixed: the public re-exports `contexts_of`,
  `family_groups`, `mechanical_groups`, `name_words`, `Groups`, `EntryOwners` (added for `context_map_measure` and
  `route_owner_measure`) are private imports again; `async-trait` (used only by `code_read_overlap`) left the
  dev-dependencies; ADRs 0028 and 0029 keep the original example name with a dated note (an accepted ADR is
  immutable); the ledger (outside git) notes `form_pairs_measure` and `code_read_overlap` as deleted.
- checks: `just check` passed; `cargo clippy -p retrodoc-pipeline --lib -D warnings` clean.
- red: none (no behaviour change).
- Code review: no findings
