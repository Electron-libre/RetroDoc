# Leave unjudged kinds out of the replay tables and the printed benchmark summary

# Goal

Since `issues/judge_silently_proposes_no_feature_pair.md`, a kind (domains or features) the judge proposed no
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
* `issues/judge_silently_proposes_no_feature_pair.md` (the measure and the decisions).

# Hints

* A saved `judge.yaml` without the mark loads as judged: do not derive the mark from the pairs at scoring time
  (decision of the parent issue).
