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
