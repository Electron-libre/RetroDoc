# Leave unjudged kinds out of the replay tables and the printed benchmark summary

# Goal

Since `issues/done/judge_silently_proposes_no_feature_pair.md`, a kind (domains or features) the judge proposed no
pair for, twice, is marked `unjudged` in `judge.yaml` and has no judged recall and precision in `run-<n>.json`,
so `benchmark-table` leaves it out of the judged mean. Two other readers of the judgement still show it at the
strict floor: the per-pass replays and the text `retrodoc benchmark` prints. A replay series should not move by
tens of points because the judge answered nothing, and the printed summary should agree with the saved report.

# Findings

* 2026-10-28, `code-review high` of `judge_silently_proposes_no_feature_pair`: `replay_domains` and
  `replay_features` (`crates/retrodoc-pipeline/src/benchmark/replay.rs`, `RunFigures.judged`, built around
  line 198) score with the judge's pairs and ignore `Judgement::unjudged`; `replay_table` averages such a run in
  with no note. The first sighting of the empty judgement was a features replay
  (`issues/done/brief_injection_experiments.md`, runs 2 and 3 at 17% and 21% against 67%).
* Same review: `summary` (`crates/retrodoc-pipeline/src/benchmark/summary.rs`, "With the judge's proposals")
  prints the judged recall and precision of an unjudged kind, while the `--out` report holds `null` for them.
* The judge already warns at `warn` in both paths, so the run log tells it; only the figures mislead.

# Approach

1. Replays: give `RunFigures.judged` no recall and precision for an unjudged kind and count the unjudged runs
   under `replay_table`, as `benchmark-table` does.
2. Summary: print the kind as unjudged (the judge proposed no pair, twice) instead of its figures.

# Specs

* `specs/benchmark-scoring.md`: extends BSC-13 to the replay tables and the printed summary, or adds a
  requirement next to it.

# Resources

* `crates/retrodoc-pipeline/src/benchmark/replay.rs`, `summary.rs`, `table.rs` (`RunReport::new`, the unjudged
  note), `judge.rs` (`Unjudged`).
* `issues/done/judge_silently_proposes_no_feature_pair.md` (the measure and the decisions).

# Hints

* A saved `judge.yaml` without the mark loads as judged: do not derive the mark from the pairs at scoring time
  (decision of the parent issue).

# Tracking

Plan: validated

## Understanding
An unjudged kind (BSC-12) must read as unjudged, not at the strict floor, in the per-pass replay tables and in the
summary `retrodoc benchmark` prints, as `benchmark-table` already does (BSC-13). Out of scope: re-scoring saved
replays, changing the judge.

## Plan
1. [x] Replays and summary: `RunFigures` carries the unjudged mark of its kind and no judged score then;
   `replay_table` shows `unjudged`, leaves it out of the judge mean and range and counts it under the table;
   `summary` prints the unjudged kind as unjudged; verified by unit tests in `replay.rs` and `summary.rs`; makes
   true BSC-15 (~150 lines).

## Decisions
- Mode: autonomous
- Base commit: b48b59d15d3f6d29153d188f0f5f9d71288c07eb

## Deliverable 1

done: `RunFigures.unjudged`, the replay table shows `unjudged` and counts it, the printed summary shows the kind
as unjudged; BSC-15 added, spec `accepted`; `docs/crates/pipeline.md`, `docs/crates/render-mcp-cli.md`.

| check | result |
|---|---|
| tests | pass |
| lint (fmt, clippy) | pass |
| specs | pass |
| harness | fails: `modified: native review gate`, unrelated (uncommitted `source` change of `.claude/harness.toml`) |
| docs | updated |
| code review | low, see below |

red: BSC-15: error[E0609]: no field `unjudged` on type `replay::RunFigures`

Code review: no findings

not verified: no real replay run where the judge answers no pair twice.

decisions needed: none
