---
priority: low
depends_on: [estimate_passes_of_the_run_path]
why: 'a bad setting on a pass turned off stops a run that never calls it; rare, and the error names the pass'
---
# Open only the providers of the passes the run calls, looked up by their constant

# Goal
`generate` opens the providers of the passes of its path (SET-9), but also those of the passes the run turns off,
so a bad `[llm.passes.<pass>]` of a pass that never runs stops it. The provider lookups name their pass by hand, so
a rename only fails at run time, during a paid run. Expected: the providers opened follow the passes the run calls,
and every lookup uses the constants of `commands/passes.rs`.

# Findings
* 2026-11-05 (the `high` review of `issues/done/estimate_passes_of_the_run_path.md`): `PassProviders::open` takes
  `passes_of(domains_from)` minus `business-read`; the passes turned off (`[pipeline] rules = false`,
  `--no-confidence`, `[pipeline] reconcile = false` or a repo without Markdown docs) are still opened, read from
  the code, not reproduced. The estimate already computes these passes (`off` in `generate/estimate.rs::print`).
* `PassProviders::get("...")` and `Run::concurrency("...")` in `generate/` and `commands/brief.rs` use string
  literals; a name missing from the opened list fails with `no LLM provider opened for the <pass> pass` when the
  pass starts.

# Approach
1. Share the list of the passes turned off between the estimate and `PassProviders::open` (one function of the
   run's settings).
2. The constants of `commands/passes.rs` at every `get` and `concurrency` site.

# Specs
* `specs/pipeline-settings.md`: SET-9 extended to the passes the run turns off (or a new requirement).

# Milestone
Pre-run cost estimate

# Resources
* `crates/retrodoc-cli/src/commands/generate/mod.rs` (`passes_of`, `TRACKED_PASSES`), `generate/estimate.rs`
  (`print`), `crates/retrodoc-cli/src/commands/usage.rs` (`PassProviders`), `commands/passes.rs`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
`generate` opens the providers of the passes it calls only: those of its path minus those turned off (rules,
confidence, reconcile), one list shared with the estimate; every `get`/`concurrency` site (generate, `brief`,
`business-read`) names its pass by a constant of `commands/passes.rs`. Out of scope: the other commands' pass lists.

## Plan
1. [x] Open only the passes the run calls: a `passes_off` function of the run's settings shared by the estimate and
   `PassProviders::open`, the reconcile returning before its provider when there are no project docs, the constants
   at every `get` and `concurrency` site; verified by binary tests (`rules = false` and `--no-confidence` with an
   unset API key on that pass: `generate` succeeds); makes true SET-9 (extended). (autonomous)

2. [x] Fixes of the `high` review: the passes turned off decided once and carried by `Run` (the rules and
   reconcile guards read them), the `set_pass` names of the one-pass commands on the constants, the reconcile
   tests and scenario of SET-9, the crate doc; verified by `tests/pass_models.rs`; makes true SET-9. (autonomous)

## Decisions
- Mode: autonomous
- Spec: SET-9 extended rather than a new requirement (same rule).
- Base commit: 0b989891ef84beafee4f335b0db76b765aec3422

## Summary of deliverable 1
- done: `generate::passes_off` (confidence, rules, reconcile) feeds both the estimate and `PassProviders::open`;
  the reconcile returns before its provider without project docs; every provider lookup and concurrency names
  its pass by a constant of `commands/passes.rs` (`generate/`, `brief`, `business-read`).
- red: SET-9: `Error: LLM settings of the `rules` pass ... environment variable RETRODOC_TEST_UNSET_KEY is not set`
- Code review: no findings
- not verified: no run on a real repository (no behaviour change for a run whose settings all open).

## Summary of deliverable 2 (fixes of the `high` review)
- done: `Run::off` carries `passes_off`, the rules and reconcile guards test it (a doc added during the run no
  longer reaches an unopened provider); `set_pass` of `brief`, `business-read`, `glossary`, `entry-points`,
  `actors`, `roles` on the constants; two SET-9 tests for the reconcile pass; scenario and crate doc.
- red: SET-9: `reconcile = false` with docs failed at the base commit with `LLM settings of the `reconcile` pass`
  (the test of the docs case guards the provider still opened, green before and after).
- High review: 8 findings. Fixed: 1, 2 (`Run::off`), 5 (`set_pass`), 6 (tests), 7 (scenario), 8 (crate doc).
  Not fixed: 3, stale judgements kept when every doc is gone: harmless, the report matches them against the
  current docs and a verdict is keyed by the section text; 4, the docs read again at the reconcile: older than
  this issue, and the drift it caused is gone with `Run::off`.
- Code review (low): 1 finding, not fixed: a second scenario tagged SET-9 is the convention of `specs/README.md` (a
  scenario names the requirement it illustrates; several per requirement elsewhere, AUD-1, AUD-13).
