---
id: EST
status: accepted
adrs: [0015]
---

# Run estimate

## Intent

Tell the user, before `generate` makes its first LLM call, how many calls and tokens the run will cost per pass and
in total, as a range extrapolated from the ratios measured on saved runs (`crates/retrodoc-pipeline/data/pass-ratios.yaml`),
so that a costly run is seen before it is paid for. Tokens and calls only, no price (ADR 0015).

## Requirements

- **EST-1** (deterministic) `generate` must print the estimate before its first LLM call: per LLM pass in the order
  of the run, then in total, a low and a high bound of the calls, the prompt tokens and the completion tokens.
- **EST-2** (deterministic) `generate --estimate` must print the estimate and stop: no LLM call, no file written
  under `.retrodoc/` nor under the docs dir.
- **EST-3** (deterministic) The estimate must be computed without any LLM call, from what the run knows before it:
  the source files the walk found, with the languages a saved `roles.yaml` promotes (within the file budget on the
  `domains_from = "contexts"` path only: on the surface path every entry point is read whatever the budget), whether
  the repo has Markdown docs of its own, the settings of the run, and the ratios of the measured table.
- **EST-4** (deterministic) A pass that scales with its input must be estimated as the number of source files to
  read times its measured cost per source file, its bounds the lowest and the highest application of the table; a
  fixed-cost pass as its cost per run, with the same bounds.
- **EST-5** (deterministic) A pass the run turns off must be estimated at zero: the confidence pass with
  `--no-confidence`, the rules pass with `[pipeline] rules = false`, the reconcile pass with `[pipeline] reconcile =
  false` or when the repo has no Markdown docs of its own, the business-read pass with `[pipeline] business_read =
  false` (the glossary and the entry points then read their files apart, which no measured run did: they are named
  as not measured, at zero).
- **EST-6** (deterministic) With `--confidence-sample N`, the high bound of the confidence pass must add `N` calls,
  each costed at the measured tokens of one confidence call.
- **EST-7** (deterministic) The estimate must be zero calls and zero tokens for every pass on a rerun without
  `--force` whose source files all have the hash saved by the last complete `generate`.
- **EST-8** (deterministic) The estimate must cost the passes that scale with their input on the changed files only
  on a rerun without `--force` where some source files changed, were added or were removed since the last complete
  `generate`, and a fixed-cost pass from zero when its saved artifact exists (reused while its own fingerprint
  holds, which the estimate can't tell).
- **EST-9** (deterministic) The printed estimate must say that its figures are extrapolated from measured ratios,
  name the models that answered the measured runs next to the models the run asks (a server may answer under
  another name than the one asked, so they are not compared), and name any pass the table has no ratio for
  (estimated at zero calls).
- **EST-10** (deterministic) The table of ratios must be embedded in the binary: a missing or unreadable table is a
  build or test failure, never a run-time one.

## Scenarios

- (EST-1) Given a repository never documented, when `generate` runs, then the estimate is printed before the line of
  the first pass, each pass with its range, then the total.
- (EST-2) Given a repository never documented, when `generate --estimate` runs, then the estimate is printed and
  `.retrodoc/cache/` is not created.
- (EST-4) Given 200 source files and a table where `use-cases` costs between 0.5 and 1.5 calls per source file, when
  the estimate is made, then `use-cases` is estimated between 100 and 300 calls.
- (EST-5) Given `--no-confidence`, when the estimate is made, then the confidence pass is at zero.
- (EST-7) Given a `generate` that went through every pass and nothing changed since, when `generate --estimate`
  runs, then every pass and the total are at zero.
- (EST-8) Given 10 of 200 source files changed since the last complete run, when the estimate is made, then the
  passes that scale are estimated on 10 files and `brief` from zero calls.

## Non-goals

- Pricing: no amount of money is shown (ADR 0015).
- A threshold or a confirmation before the run: the estimate is a display.
- Predicting what a new binary (changed prompts) or another model redoes on a rerun: the printed estimate says so.
- Comparing the estimate with what the run cost: `specs/token-accounting.md` (TOK-6 to TOK-8).
