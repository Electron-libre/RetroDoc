---
priority: high
depends_on: [estimate_against_actual]
why: 'the estimate leaves out the joint reading of the business files, on by default, so it underestimates nearly every run'
---
# Estimate the business-read pass of a run

# Goal
The pre-run estimate is built from `GENERATE_PASSES`, the list of the passes that open an LLM provider. The joint
reading of the business files (`business-read`) has no provider of its own (it calls those of `glossary` and
`entry-points`), so it is not in that list and is never estimated, although the ratio table measures it and
`generate` tracks its calls under that name. Expected: with `[pipeline] business_read = true` (the default), the
estimate gives `business-read` its range in the order of the run and counts it in the total, and the end-of-run
recap sets it against that range; with `business_read = false`, the pass shows as off.

# Findings
* 2026-11-05 (the `high` review of `issues/done/estimate_against_actual.md`): `GENERATE_PASSES`
  (`crates/retrodoc-cli/src/commands/generate/mod.rs`) has no `business-read`; `data/pass-ratios.yaml` has a
  `business-read` row; `generate` runs `tracker.in_pass("business-read", ...)` (`generate/mod.rs`,
  `generate/contexts.rs`); `off.push("business-read")` in `generate/estimate.rs` has no effect, the pass not being
  in the estimated list.
* So the estimate total leaves out its calls and tokens (EST-1), and the recap never sets it against the estimate
  (TOK-7).

# Approach
1. Estimate the passes as `generate` tracks them: a list of the estimated passes apart from the provider list
   (`business-read` before `glossary`), or `business-read` inserted when building the estimate inputs.
2. A CLI test: the estimate block names `business-read` with a range when it is on, `off in this run` when
   `business_read = false`.

# Specs
* `specs/run-estimate.md`: EST-1 (every LLM pass of the run in its order), EST-5 (the pass off).

# Milestone
Pre-run cost estimate

# Resources
* `crates/retrodoc-cli/src/commands/generate/estimate.rs`, `crates/retrodoc-cli/src/commands/generate/mod.rs`
  (`GENERATE_PASSES`), `crates/retrodoc-pipeline/src/estimate.rs`, `specs/run-estimate.md`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
The estimate lists the passes as `generate` tracks them: `business-read` inserted before `glossary`, estimated per
source file from its ratio row, counted in the total, `off in this run` with `business_read = false`; its model is
the one of `glossary` (and of `entry-points` when it differs), whose providers it calls. No spec change: EST-1 and
EST-5 already require it.

## Plan
1. [x] Estimate `business-read`: the estimated passes built apart from `GENERATE_PASSES` in
   `generate/estimate.rs`, `business-read` before `glossary`; verified by CLI tests in `tests/run_estimate.rs` (the
   block names `business-read` with a range when on, `off in this run` with `business_read = false`, the recap sets
   it against the estimate); makes true EST-1, EST-5, TOK-7. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 4fd92251948eccb07ad1864ca247e589d9d8c970
- Model shown for `business-read`: the one of `glossary`, plus the one of `entry-points` when it differs (1 a).

## Deliverable 1 summary
- done: `business-read` estimated before `glossary`, in the total and the recap comparison, `off in this run` with
  `business_read = false`; its models are those of `glossary` and `entry-points`.
- red: EST-1, EST-5, TOK-7: `business-read estimated` / `business-read named` (no `business-read` line in the estimate).
- Code review: 1 findings: business-read both listed and off with `business_read = false`; not fixed, EST-5 wants it named `off in this run` like `confidence`, its models left out by `render`.
- not verified: a real run (the ratio row is the measured one; no paid run made).
