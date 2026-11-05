---
priority: medium
depends_on: [estimate_business_read]
why: 'a guard so the next pass with no provider of its own is not missed again; the known gap is fixed'
---
# Show the measured passes the estimate leaves out in the recap

# Goal
The end-of-run recap sets against the estimate only the passes the estimate holds (`usage_log::recap_against`
walks `estimate.passes`). A pass that `generate` tracks under an `in_pass` name the estimate doesn't hold, so with
real calls, is left out of "Against the estimate" with no warning, and the estimate total underestimates silently:
that is what hid `business-read`. Expected: such a pass shows in the comparison as not estimated, and the list of the
estimated passes can't drift from the passes `generate` tracks.

# Findings
* 2026-11-05 (the `high` review of `issues/done/estimate_business_read.md`): `recap_against`
  (`crates/retrodoc-pipeline/src/usage_log.rs`) builds its rows from `estimate.passes` only; a pass in
  `report.passes` absent from the estimate is never printed there.
* `estimated_passes` (`crates/retrodoc-cli/src/commands/generate/estimate.rs`) copies by hand the order of the
  `in_pass` calls of `generate/mod.rs` and `generate/contexts.rs` (`business-read` inserted before `glossary`);
  nothing ties the two.

# Approach
1. In `recap_against`, print each pass of `report.passes` with calls that the estimate doesn't hold as
   `not estimated`.
2. Optionally, one list of the tracked passes next to `GENERATE_PASSES`, from which the provider list is derived,
   so the estimate and the run share their order.

# Specs
* `specs/token-accounting.md`: TOK-7 (the recap set against the estimate), amended for a pass not estimated.

# Milestone
Pre-run cost estimate

# Resources
* `crates/retrodoc-pipeline/src/usage_log.rs` (`recap_against`), `crates/retrodoc-cli/src/commands/generate/estimate.rs`,
  `crates/retrodoc-cli/src/commands/generate/mod.rs` (`GENERATE_PASSES`), `specs/token-accounting.md`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
The recap must show, as `not estimated`, a pass that made calls and that the estimate doesn't hold, and the estimate
must take its pass list from the same list as `generate`. Out of scope: estimating such a pass.

## Plan
1. [x] Recap: after the rows of the estimate, each pass of `report.passes` with calls absent from `estimate.passes`
   gets a row `not estimated`; verified by a unit test in `usage_log.rs`; makes true TOK-7 (amended). (autonomous)
2. [ ] One list: `TRACKED_PASSES` in `generate/mod.rs` (the order of the `in_pass` calls, `business-read` before
   `glossary`), the providers opened from it minus `business-read`, `estimated_passes` filtering it instead of
   inserting by hand; verified by the existing estimate tests and a test that `estimated_passes(true)` equals
   `TRACKED_PASSES`; refactor, no requirement.

## Decisions
- Mode: autonomous
- Base commit: b7695374f6c5928a8de5ce04896881feed38d27b
- Deliverable 2 kept (question 1 a).

## Deliverable 1
done: `recap_against` adds a `not estimated` row for each pass with calls the estimate doesn't hold; TOK-7 amended.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass |
| docs | `specs/token-accounting.md`, `docs/crates/pipeline.md` |
| code review | clean |

red: TOK-7: `left: "    brief  within the range\n"` (no `business-read  not estimated` row)

Code review: no findings
