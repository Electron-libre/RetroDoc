---
id: EST
status: accepted
adrs: [0015, 0056]
---

# Run estimate

## Intent

Tell the user, before `generate` makes its first LLM call, how many calls and tokens the run will cost per pass and
in total, as a range extrapolated from the ratios measured on saved runs (`crates/retrodoc-pipeline/data/pass-ratios.yaml`),
so that a costly run is seen before it is paid for. Tokens and calls only, no price (ADR 0015).

## Requirements

### Behaviour

- **EST-1** (deterministic) `generate` must print the estimate before its first LLM call: per LLM pass in the order
  of the run, then in total, a low and a high bound of the calls, the prompt tokens and the completion tokens.
- **EST-2** (deterministic) `generate --estimate` must print the estimate and stop: no LLM call, no file written
  under `.retrodoc/` nor under the docs dir.
- **EST-7** (deterministic) The estimate must be zero calls and zero tokens for every pass on a rerun without
  `--force` whose source files all have the hash saved by the last complete `generate`.
- **EST-8** (deterministic) The estimate must cost the passes that scale with their input on the changed files only
  on a rerun without `--force` where some source files changed, were added or were removed since the last complete
  `generate`, and a fixed-cost pass from zero when its saved artifact exists (reused while its own fingerprint
  holds, which the estimate can't tell).
- **EST-12** (deterministic) On a rerun without `--force`, the estimate must count the units a review `wrong` not
  yet answered asks again (HCR-4), each at the measured cost per unit of its pass: one domains call when a domain is
  reviewed, one features call per domain that holds a reviewed feature, one use cases call per feature that holds a
  reviewed use case; a review its unit already answered adds nothing, nor does a feature with no entry point, which
  no call of the features is shown.

### Contract

- **EST-3** (deterministic) The estimate must be computed without any LLM call, from what the run knows before it:
  the source files the walk found, with the languages a saved `roles.yaml` promotes (within the file budget on the
  `domains_from = "contexts"` path only: on the surface path every entry point is read whatever the budget), whether
  the repo has Markdown docs of its own, the settings of the run, the reviews `wrong` of `.retrodoc/reviews.json` its saved units have not answered yet
  (`specs/human-corrections.md`, HCR-4, HCR-10), and the ratios of the measured table.
- **EST-4** (deterministic) A pass that scales with its input must be estimated as the number of source files to
  read times its measured cost per source file, its bounds the lowest and the highest application of the table; a
  fixed-cost pass as its cost per run, with the same bounds.
- **EST-5** (deterministic) A pass the run turns off must be estimated at zero: the confidence pass with
  `--no-confidence`, the rules pass with `[pipeline] rules = false`, the reconcile pass with `[pipeline] reconcile =
  false` or when the repo has no Markdown docs of its own, the business-read pass with `[pipeline] business_read =
  false` or when the glossary and the entry points ask two models (the joint reading needs one) (the glossary and the entry points then read their files apart, which no measured run did: they are named
  as not measured, at zero).
- **EST-6** (deterministic) With `--confidence-sample N`, the high bound of the confidence pass must add `N` calls,
  each costed at the measured tokens of one confidence call.
- **EST-9** (deterministic) The printed estimate must say that its figures are extrapolated from measured ratios,
  name the models that answered the measured runs next to the models the run asks (a server may answer under
  another name than the one asked, so they are not compared), and name any pass the table has no ratio for
  (estimated at zero calls).
- **EST-10** (deterministic) The table of ratios must be embedded in the binary: a missing or unreadable table is a
  build or test failure, never a run-time one.
- **EST-11** (deterministic) The estimate must hold only the passes of the path the run takes (`[pipeline]
  domains_from`): `contexts` on the `contexts` path only, `business-files` and `domains` on the `surface` path only.
- **EST-13** (deterministic) A feature or a use case a review asks again must also count the calls its answer
  cascades into: for a reviewed use case, the rules of its feature and the confidence of the use cases saved for that
  feature; for a reviewed feature, the use cases and the rules of that feature and the confidence of its saved use
  cases; a domains call asked again counts nothing downstream (as a domains call asked by a changed file), and the
  reconcile pass nothing for any review.
- **EST-14** (deterministic) `generate --estimate` must read the human corrections as `generate` does: it fails
  with the error of HCR-11 when `.retrodoc/edits.json` cannot be read, and goes on with no review when
  `.retrodoc/reviews.json` cannot be read.

## Scenarios

- **EST-H1** (happy; EST-1, EST-12, EST-13) Given a documented repository with nothing changed since and a use case
  the team marked `wrong`, when `generate` runs, then the estimate printed before the first call counts the use cases
  call of its feature and the rules and confidence calls it cascades into, every other pass at zero.
- (EST-1) Given a repository never documented, when `generate` runs, then the estimate is printed before the line of
  the first pass, each pass with its range, then the total.
- (EST-2) Given a repository never documented, when `generate --estimate` runs, then the estimate is printed and
  `.retrodoc/cache/` is not created.
- (EST-4) Given 200 source files and a table where `use-cases` costs between 0.5 and 1.5 calls per source file, when
  the estimate is made, then `use-cases` is estimated between 100 and 300 calls.
- (EST-5) Given `--no-confidence`, when the estimate is made, then the confidence pass is at zero.
- (EST-11) Given `domains_from = "contexts"`, when `generate --estimate` runs, then `contexts` is estimated and
  neither `business-files` nor `domains` is named; given the `surface` path, `contexts` is not named.
- (EST-7) Given a `generate` that went through every pass and nothing changed since, when `generate --estimate`
  runs, then every pass and the total are at zero.
- (EST-8) Given 10 of 200 source files changed since the last complete run, when the estimate is made, then the
  passes that scale are estimated on 10 files and `brief` from zero calls.
- (EST-12) Given a use case marked `wrong` that a `generate` already answered and nothing changed since, when
  `generate --estimate` runs, then every pass is at zero.
- (EST-12) Given a domain marked `wrong` and nothing changed, when the estimate is made, then `domains` is estimated
  at one call and the passes downstream at zero.
- (EST-12) Given two features of one domain marked `wrong`, when the estimate is made, then `features` counts one call
  of that domain.
- (EST-12) Given a review `wrong` and `--force`, when the estimate is made, then every file is costed as on a first
  run and the review adds nothing more.
- (EST-14) Given an unreadable `.retrodoc/edits.json`, when `generate --estimate` runs, then it fails naming the file.

## Non-goals

- Pricing: no amount of money is shown (ADR 0015).
- A threshold or a confirmation before the run: the estimate is a display.
- Predicting what a new binary (changed prompts) or another model redoes on a rerun: the printed estimate says so.
- Predicting how far a domains call asked again by a review moves the domains, features and use cases after it: the
  cascade of a changed domain is not counted, as for a changed file.
- Telling, on the `contexts` path, the reviews of the contexts the run leaves out: the slice a run documents is
  planned from the context map after the estimate, so the reviews of every context are counted.
- Comparing the estimate with what the run cost: `specs/token-accounting.md` (TOK-6 to TOK-8).
