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
* 2026-11-05 (the `high` review of `issues/estimate_passes_of_the_run_path.md`): `PassProviders::open` takes
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
