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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
Move `src/benchmark/`, the 22 examples and `tests/benchmark_files.rs` to a new crate `retrodoc-bench` (approach a),
no change of behaviour of `generate`, `benchmark`, `benchmark-table` or the examples. Out of scope: deleting the
one-off examples (a follow-up issue, the user picks them). Means: pure `git mv` (counted 0 by `deliverable-size`,
measured), the reached private items exposed behind a `bench` feature, `crate::<module>` aliases at the new root.

## Plan
1. [x] Crate `retrodoc-bench` with the examples and their test: create the crate (depends on `retrodoc-pipeline`),
   `git mv` the 22 examples and `tests/benchmark_files.rs` unchanged (they import `retrodoc_pipeline::benchmark`,
   still valid), move the `[[example]]` entries; verified by `just check` (examples built and tested under the new
   crate); est. ~120 lines. (autonomous)
2. [x] `benchmark/` in `retrodoc-bench`: `git mv src/benchmark/` unchanged; in `retrodoc-pipeline` a `bench` feature
   with a `#[doc(hidden)] pub mod bench_support` (naming, text, progress, response, artifact, walked, the rules
   judgement items) and a `testing` feature for the fake LLM; the new root aliases the modules; the 5 files that
   import `retrodoc_pipeline::benchmark` and `retrodoc-cli` switch to `retrodoc_bench`; verified by `just check`
   (the benchmark unit tests run in the new crate) and `retrodoc benchmark-table` on a saved benchmark dir giving
   the same output as before; est. ~250 lines in the deliverable, ~2,450 in the issue total (5 moved files edited,
   counted twice). (autonomous)
3. [ ] Docs: crate graph of `CLAUDE.md` and `docs/ARCHITECTURE.md`, `docs/crates/` (a section for the new crate),
   `docs/dev/commands.md` (`-p retrodoc-bench`), skills `quality-benchmark`, `smoke-test`, `commit_scopes` of
   `.claude/cardinal.toml` (`bench`), ADR 0042 accepted, the reuse rule of the ledger (outside git); verified by
   `just check` and a grep of `retrodoc-pipeline --example` and `retrodoc-pipeline/src/benchmark` left nowhere;
   est. ~150 lines.

Examples named nowhere among skills, `docs/dev/commands.md`, the ledger's measures or an open issue (candidates to
delete in the follow-up): `confidence_agreement` (ADR 0026 only), `orphan_placement_measure`, `read_overlap` (ADR
0028 only). Named only by `docs/dev/commands.md`, one issue each, none in the planned measures: `behaviour_baseline`,
`features_by_domain_measure`, `public_api_reach_measure`, `route_owner_measure`, `rules_answer_size_measure`,
`rules_noise_measure`, `technical_usecases_measure`, `use_cases_answer_size_measure`, `parsed_slices`,
`context_map_measure`, `routing_duplicates_measure`, `form_pairs_measure`, `code_read_overlap`,
`narrative_check_measure`. Kept as tools: `pass_replay`, `freeze_inputs`, `entry_points_replay`,
`rules_judge_replay`, `placement_replay`.

## Decisions
- Mode: autonomous
- Base commit: 25efa2d9fab4a5a043dc4fd635c144f1f2eb4378
- One-off examples: deleted in a follow-up issue, the user picks them (1 a).
- Moved files keep their `crate::<module>` paths through aliases at the root of `retrodoc-bench` (2 a).

## Deliverable 1
done: crate `retrodoc-bench` created, the 22 examples and `tests/benchmark_files.rs` moved unchanged, the commands
of `docs/dev/commands.md`, the skill `quality-benchmark`, `docs/crates/pipeline.md`, the `entry-points.yaml`
references and the ledger's reuse rule name `-p retrodoc-bench`.

| check | result |
|---|---|
| tests | `just check` green, the examples' tests run under `retrodoc-bench` |
| lint | clean |
| specs | untouched (refactor) |
| docs | commands, skill, crate doc updated; the crate graph waits for deliverable 3 |
| code review | `low`, clean |

red: none (a move, no new behaviour; the moved tests are the check)

Follow-up: the usage comment at the top of each moved example still says `-p retrodoc-pipeline`; fixing it edits
moved files, counted twice: it goes to the follow-up issue that deletes the one-off examples.

Code review: no findings

## Deliverable 2
done: `benchmark/` moved unchanged to `retrodoc-bench`, its root aliases the modules of `retrodoc-pipeline`;
`retrodoc-pipeline` gains the features `bench` (`bench_support`, `rules::bench`, out of the default build) and
`testing` (`FakeLlm`, `brief`, `capture_logs`); `judge_rules_with` moved into `rules::bench` and `truncate_chars`
gated, both only served the replays; `UNCATEGORIZED_SLUG` public; `retrodoc-cli` and 5 files import `retrodoc_bench`.

| check | result |
|---|---|
| tests | `just check` green (1,117 tests), the benchmark unit tests run in `retrodoc-bench` |
| lint | clean in the workspace and for `retrodoc-pipeline` alone, with and without `bench` |
| specs | untouched (refactor) |
| behaviour | `benchmark-table` of the two rescored baselines of `dafea18`, alone and with `--previous`: output identical to the binary of `c7e765d` |
| size | 206 lines; issue total 2,790 of 3,000 |
| code review | `low`, clean |

red: none (a move, no new behaviour)

Code review: no findings
