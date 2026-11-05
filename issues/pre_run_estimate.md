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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
Before the first LLM call of `generate` (and alone with a new `--estimate`), a low-high range of calls, prompt and
completion tokens per pass and in total, from the source files to read and the measured table, zero for what the
run turns off or the cache saves. Out of scope: prices, a confirmation, the comparison with the actual cost.
Means: each pass that scales is costed per source file (one measured range per pass), not by chaining the yields.

## Plan
1. [x] The table gains the cost of each pass per source file (the files of `source-hashes.json` of each run) and is
   rebuilt from the same runs, no LLM; verified by the tests of `pass_ratios` and a byte-identical rebuild (autonomous).
2. [ ] `retrodoc_pipeline::estimate`: the table embedded and read, a pure estimate from the inputs of EST-3 (files
   to read, files changed since the last complete run, passes turned off, confidence sample, docs present, `force`);
   verified by unit tests; makes true EST-4, EST-5, EST-6, EST-7, EST-8, EST-10.
3. [ ] `generate`: the estimate printed before the first call, `--estimate` that stops there, the rerun inputs read
   from `source-hashes.json` and the saved artifacts; docs; verified by CLI tests; makes true EST-1, EST-2, EST-3,
   EST-9.

## Decisions
- Mode: autonomous
- Base commit: 00fcf8ea083aee14d565a07c66af1d4bee3de89b
- `--estimate` prints the estimate and stops; `--dry-run` keeps its meaning (it still runs the LLM passes) (user, 2026-11-05).
- Each pass that scales is costed per source file, measured directly, bounds the lowest and highest application;
  the yields are not chained (user, 2026-11-05).
- `domains_from = "contexts"`: the same estimate on the files the run reads (its budget), the `contexts` pass
  named as unmeasured (user, 2026-11-05).
- A rerun with changed files: the passes that scale on the changed files only; the display says a new binary or
  another model redoes more (user, 2026-11-05).

## Deliverable 1
- Code review: no findings (`low`).
- done: `Units::source_files` (from `source-hashes.json`), `per_source_file` per application and spread per pass,
  yield `source files per run`; table rebuilt from the same 22 runs (byte-identical twice); dated finding in
  `docs/findings.md` §3 point 7; `bench.md`, `pipeline.md`, `commands.md`.
- checks: tests 11/11 of `pass_ratios`, `cardinal run project-check` green, docs updated.
- red: none (a measure, no spec requirement): the new assertions failed to compile before the fields existed.
- not verified: the per-source-file spread is wide (`use-cases` 0.09-0.94 calls per file); the small Ruby library
  (9 files) gives every upper bound.
