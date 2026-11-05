---
priority: medium
depends_on: [recap_passes_not_estimated]
why: 'the contexts path is overestimated and its recap reports false misses on every run; the surface path is right'
---
# Estimate and open only the passes of the path the run takes

# Goal
`TRACKED_PASSES` (`generate/mod.rs`) lists every pass of both paths; the estimate drops only `contexts` on the surface
path, and the providers are opened for all of them. On the contexts path (`domains_from = "contexts"`),
`business-files` and `domains` make no LLM call, yet the estimate counts them, so the total is too high and the recap
sets them against their range as misses. Expected: the estimate and the opened providers follow the passes of the
path the run takes, and the `in_pass` names can't drift from the list.

# Findings
* 2026-11-05 (the `high` review of `issues/recap_passes_not_estimated.md`): `contexts.rs` tracks `contexts`, `actors`,
  `features`, `use-cases`, `business-read`, `glossary` and `entry-points` (plus `roles` through
  `identify_file_roles`); it computes its domains locally (`domains_from_contexts`) and never runs `business-files`.
  The estimate already counted those before `TRACKED_PASSES` (pre-existing).
* `PassProviders::open` builds a provider for every pass of the list, so a bad `[llm.passes.contexts]` stops a
  surface run that never calls it (not reproduced, read from the code).
* The `in_pass` call sites still use string literals besides `BUSINESS_READ`; only the recap's `not estimated` row
  catches a forgotten one, after the run.
* Not checked: whether `rules`, `confidence` and `reconcile` run on the contexts path.

# Approach
1. Measure: list the `in_pass` names each path reaches (a test, or a scripted run on each path).
2. One list per path (or the list with the paths each pass belongs to), used by the estimate and `PassProviders::open`.
3. Named constants (or the list's entries) at the `in_pass` sites.

# Specs
* `specs/run-estimate.md`: the passes estimated per path, to check whether a requirement covers it.

# Milestone
Pre-run cost estimate

# Resources
* `crates/retrodoc-cli/src/commands/generate/mod.rs` (`TRACKED_PASSES`), `generate/contexts.rs`, `generate/estimate.rs`
  (`estimated_passes`), `crates/retrodoc-cli/src/commands/usage.rs` (`PassProviders::open`).
