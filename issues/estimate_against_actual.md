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
