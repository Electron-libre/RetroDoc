---
priority: high
depends_on: [pass_ratios_from_usage_history]
why: 'the goal of the milestone: know what a run costs before paying for it'
---
# Estimate the calls and tokens of a generate before its first LLM call

# Goal
`generate` says what a run cost only at its end (the token recap). Nothing estimates it before: the estimate of the
repo map pass was removed with that pass (`d5255d5`). Expected: before its first LLM call, and with `--dry-run`,
`generate` prints a range of calls and tokens (prompt, completion) per pass and in total, from what it knows (the
inputs it counts, the units the cache saves) and the measured ratios of `pass_ratios_from_usage_history` for what the
earlier passes will produce. Tokens and calls only, no amount (ADR 0015); a display, no confirmation.

# Findings
* 2026-11-05: decided with the user: tokens and calls only, no price; no threshold that asks for a confirmation.
* 2026-11-05: the passes after the surface depend on its output (contexts, domains, features, use cases): their
  units are not known before the run and must be extrapolated; output tokens are the hardest to predict
  (`docs/findings.md` §3: off by 2-3x before any measure).
* 2026-11-05: the cache already decides which units a rerun redoes (fingerprints): a rerun with nothing changed
  should estimate zero calls.

# Approach
1. A pure estimate per pass from the counts known before the run (source files after the budget, entry point
   files, the units whose fingerprint is unchanged) and the ratios, giving a low and a high bound.
2. Print it before the first call and in `--dry-run`, in the form of the recap (per pass, total); say which figures
   are extrapolated.
3. Options to weigh at planning: `--confidence-sample`, `--no-confidence`, `[pipeline] rules = false`, the model per
   pass and `--context` change the estimate and must be read from the same settings as the run.

# Specs
* `specs/token-accounting.md`, or a new `specs/run-estimate.md`: the estimate, its bounds, a rerun with nothing to do.

# Milestone
Pre-run cost estimate

# Resources
* `crates/retrodoc-cli/src/commands/generate/`, `crates/retrodoc-cli/src/commands/usage.rs`,
  `crates/retrodoc-pipeline/src/usage_log.rs`, `docs/crates/pipeline.md` (pass order, cache, model per pass),
  ADR 0015.

# Hints
* No LLM call to make the estimate, and no new file in the docs dir (reruns stay no-ops).
