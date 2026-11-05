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
2. [x] The recap of `generate` shows, per pass, within the range or the bound missed and the miss in percent;
   verified by unit tests of `recap` and the CLI test; makes true TOK-7 (~350 lines) (autonomous).
3. [x] The `pass_ratios` measuring example reports, per pass, the runs within range and the median miss of the
   others, from the estimates of the measured runs; verified by tests of `pass_ratios`; makes true TOK-8 (~300 lines) (autonomous).
4. [x] Docs of the whole issue and fixes of its `high` review; makes true the amended TOK-6, TOK-7, TOK-8 (autonomous).

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

## Deliverable 2
- done: `usage_log::recap_against` (the recap, then `Against the estimate:` per pass estimated or that made a call:
  `within the range`, or each figure missed with its bound and the miss in percent, none against a zero bound),
  printed at the end of a counted command; `docs/crates/pipeline.md`.
- checks: tests 7/7 of `run_estimate`, 16/16 of `usage_log`, `cardinal run project-check` green, docs updated.
- red: TOK-7: the CLI test failed with the plain recap (`the comparison` not found); the unit tests failed to compile
  before `recap_against` existed.
- Code review: no findings (`low`).
- not verified: the figures of a real paid run.

## Deliverable 3
- done: `RunMeasure::estimate` (the estimate of a run's first `generate`, none for a resumed run);
  `pass_ratios::estimate_report` (per pass and figure: the runs within range, the median miss of the others), printed
  on stderr by the example `pass_ratios`; `Range::miss` and `Miss` shared with the recap; `docs/crates/bench.md`,
  `docs/dev/commands.md`.
- checks: tests 12/12 of `pass_ratios`, `cardinal run project-check` green, docs updated; spec `accepted`.
- Code review: no findings (`low`).
- red: TOK-8: the two tests failed to compile (`cannot find function estimate_report`).
- not verified: on the saved runs of `~/retrodoc-runs` (none holds an estimate yet: they predate deliverable 1).

## Review of the whole issue (`high`)
Code review: 7 findings.
- Fixed: a token count below its range when calls reported no usage is a lower bound, not a miss
  (`PassEstimate::against`, `Against::LowerBound`; TOK-7, TOK-8 amended); a failed run is neither compared nor saved
  with its estimate (TOK-6, TOK-7 amended); an estimate the binary can't read is dropped and its run kept
  (`lenient_estimate`, TOK-6 amended), so a later change of `Basis` can't wipe the history; the report splits the
  misses above and below the range, each with its median (TOK-8 amended); the comparison is shared by the recap and
  the report (`PassEstimate::is_compared`, `against`); `&mut Option` instead of a `OnceLock`.
- Not fixed here: `business-read` is never estimated (it is not in `GENERATE_PASSES`): a bug of the estimate (EST-1),
  outside this issue's requirements: `issues/estimate_business_read.md`.
- Docs of the whole issue: `README.md` (cost control), `docs/findings.md` §3 point 5, `docs/crates/pipeline.md`,
  `docs/crates/bench.md`.
- checks: tests 18/18 of `usage_log`, 14/14 of `pass_ratios`, the CLI suite, `cardinal run project-check` green;
  spec `accepted`.
- red: TOK-6, TOK-7, TOK-8: the five new tests failed before the fixes (the estimate saved with a failed run, the
  history with an unreadable estimate loaded as empty, no lower bound in the recap nor the report, the misses not
  split).
- Fixes reviewed (`low`): Code review: no findings.
