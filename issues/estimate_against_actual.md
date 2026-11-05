---
priority: medium
depends_on: [pre_run_estimate]
why: 'keeps the estimate honest over time; the estimate is usable without it'
---
# Compare the estimate of a run with what it cost

# Goal
An estimate nobody checks drifts as the prompts and passes change. Expected: the end-of-run recap shows, per pass,
the estimated range next to the calls and tokens measured, and `usage.json` keeps both, so that the ratios of
`pass_ratios_from_usage_history` can be recomputed from the runs that follow.

# Findings
* 2026-11-05: `usage.json` (`RunUsage`) keeps the last 20 runs of a repository, per pass and model; it holds no
  estimate.

# Approach
1. Save the estimate of `pre_run_estimate` with the run in `usage.json` (an older history without it still loads).
2. In the recap, per pass: the measured figures and whether they fell inside the range, the miss when not.
3. Let the measuring tool of `pass_ratios_from_usage_history` read these records to recompute the ratios.

# Specs
* `specs/token-accounting.md`: the estimate kept in the history and shown in the recap.

# Milestone
Pre-run cost estimate

# Resources
* `crates/retrodoc-pipeline/src/usage_log.rs`, `specs/token-accounting.md` (TOK-5: an older history must load).

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
Keep the estimate of a `generate` in `usage.json`, set the measured figures of each pass against its range in the
recap, and let the measuring tool of the ratio table report how the recorded estimates held. Out of scope: correcting
the table from the misses (it stays rebuilt by hand), an estimate for the other commands.

## Plan
1. [x] The estimate of `generate` saved with its run in `usage.json` (`RunUsage::estimate`, optional), carried from
   `generate` to the end of the command; verified by unit tests of `usage_log` and a CLI test on a fake LLM; makes
   true TOK-6 (~300 lines) (autonomous).
2. [ ] The recap of `generate` shows, per pass, within the range or the bound missed and the miss in percent;
   verified by unit tests of `recap` and the CLI test; makes true TOK-7 (~350 lines).
3. [ ] The `pass_ratios` measuring example reports, per pass, the runs within range and the median miss of the
   others, from the estimates of the measured runs; verified by tests of `pass_ratios`; makes true TOK-8 (~300 lines).

## Decisions
- Mode: autonomous
- Base commit: 486944d2f3642e4a27f9f5fb6261a47730cb5b7a
- The measuring tool reports how the recorded estimates held (runs within range, median miss) next to the table,
  which stays rebuilt as today from the measured runs; no automatic correction of the table (user, 2026-11-05).

## Deliverable 1
- done: `RunEstimate`, `PassEstimate`, `Range`, `Basis` serializable; `RunUsage::estimate` (optional, absent from an
  older history); `generate` leaves its printed estimate in a `OnceLock` the end of the command saves with the run;
  `docs/crates/pipeline.md`, `ARCHITECTURE.md`.
- red: TOK-6: the CLI test failed with `estimate` absent from `usage.json` (`Null` where 2 source files were
  expected); the unit test was written with the field.
- checks: tests 6/6 of `run_estimate`, 14/14 of `usage_log`, `cardinal run project-check` green, docs updated.
- Code review: no findings (`low`).
- not verified: a real paid run (the fake LLM only).
- The end of a command (`counted`, `snapshot`, the estimate) is one `Ending` in `main.rs`: `main` was over the
  line limit of clippy.
