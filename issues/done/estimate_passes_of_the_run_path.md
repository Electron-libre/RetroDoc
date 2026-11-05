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
* 2026-11-05 (the `high` review of `issues/done/recap_passes_not_estimated.md`): `contexts.rs` tracks `contexts`, `actors`,
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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
The estimate and `PassProviders::open` follow the passes of the path the run takes; the `in_pass` sites use named
constants. Measured from the code: the contexts path runs `roles`, `contexts`, `business-read`, `glossary`,
`entry-points`, `actors`, `features`, `use-cases`, `rules`, `confidence`, `reconcile` (`finish_chain`), never
`business-files` nor `domains`; both paths start with `sources`, `brief`. Out of scope: the `llm.get` literals.

## Plan
1. [x] Passes per path: each entry of `TRACKED_PASSES` says its path (both, surface, contexts), the estimate and
   `PassProviders::open` take the passes of the run's path; verified by `generate --estimate` on each path (through
   the binary) and a surface `generate` with an unopenable `[llm.passes.contexts]`; makes true EST-11, SET-9. (autonomous)
2. [x] Named constants: one constant per tracked pass, used by the list and every `in_pass` site of `generate`
   (`generate/`, `brief.rs`, `business_files.rs`, `contexts.rs`); verified by the existing tests (pure refactor). (autonomous)
3. [x] Fixes of the `high` review of the issue: see `## Deliverable 3`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: fda71379c341f2280eaa2c8d777a38eb641c9f89

## Deliverable 1
done: each entry of `TRACKED_PASSES` names its path; `passes_of(domains_from)` feeds the estimate and
`PassProviders::open`.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | EST-11, SET-9 traced |
| docs | `docs/crates/render-mcp-cli.md`, `docs/crates/pipeline.md` |
| code review | see below |

red: EST-11, SET-9: the contexts estimate named `business-files`; the surface `generate` stopped on `LLM settings of
the `contexts` pass`.

Code review: no findings

not verified: a real run on the contexts path (no scripted test reaches it end to end).
decisions needed: none

## Deliverable 2
done: `commands/passes.rs` holds one constant per tracked pass; `TRACKED_PASSES`, the estimate and every `in_pass`
site of `generate` use them (the standalone `business-read` command's `set_pass` left as is).

| check | result |
|---|---|
| tests | pass (pure refactor) |
| lint | pass |
| specs | n/a |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: none

Code review: no findings

not verified: nothing beyond deliverable 1.
decisions needed: none

## Deliverable 3
done: fixes of the `high` review (8 findings): a SET-9 test on the contexts path; `saved_fixed` filtered by the
passes of the path; a test that every tracked pass but `business-read` is in `PASS_NAMES`; the order test moved
next to `passes_of`, its copy of the path table dropped (the binary tests cover it); the doc of `passes.rs` says the
provider lookups still name their pass.
Not fixed: providers of the passes turned off still opened, and the `get`/`concurrency` literals: outside EST-11 and
SET-9, in `issues/open_only_the_passes_the_run_calls.md`; the hand-kept path tag (a drift fails at once, naming the
pass); the `contexts` bool of `estimate::print` (it drives the budget, not the passes).

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | SET-9 traced on both paths |
| docs | n/a (no behaviour change beyond deliverable 1) |
| code review | see below |

red: SET-9: with `domains` tagged on both paths, the contexts run stopped on `LLM settings of the `domains` pass`.

Code review: no findings

not verified: nothing beyond deliverable 1.
decisions needed: none

All deliverables done; follow-up in `issues/open_only_the_passes_the_run_calls.md`.
