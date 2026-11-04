---
id: AUD
status: accepted
adrs: [0039]
---

# Audit dashboard of the docs

## Intent

The audit page of `retrodoc ui`, `/audit`, shows the figures of the last run and where its debt concentrates, domain
by kind, so whoever audits the docs decides where to improve them first, then opens the debt view
(`specs/debt-view.md`) on the cell they picked.

## Requirements

- **AUD-1** (deterministic) The audit page, `/audit`, must show the overall confidence and the overall business
  language of the run, as the debt report of `retrodoc report` computes them, each flagged low below
  `LOW_CONFIDENCE_THRESHOLD` (WEB-6), each shown when the run has it, whether or not it has the other.
- **AUD-2** (deterministic) The audit page must show the coverage of the entry points from
  `.retrodoc/cache/behaviour-coverage.yaml`: the entry points held by a feature out of those of the bounded contexts,
  and the entry points cited by a use case out of those of the features, each as a count and a percentage; only the
  domains and features of the run count.
- **AUD-3** (deterministic) The audit page must show the stale sections (the features and use cases of DBT-7) as a
  count out of the features of the run and their use cases, and a percentage rounded to the unit, but never 0 when
  the count is above 0 nor 100 when it is below the total (the same for the percentages of AUD-2).
- **AUD-4** (deterministic) The audit page must show the cost of the last `generate` saved in
  `.retrodoc/cache/usage.json`: its end date, then one row per pass that called the LLM with its duration, its calls
  and its prompt, cached and completion tokens, and a total row.
- **AUD-5** (deterministic) The audit page must show the progress of the bounded contexts: the number of contexts of
  `contexts.yaml`, those whose chain finished as `fingerprints.json` records it, and the names of the others, pending,
  in the order of the map.
- **AUD-6** (deterministic) The audit page must show a heatmap as a table with one row per domain of the run in the
  order of the run and one column per kind of the debt view in the order of DBT-8, each cell holding the number of
  debt items of that kind in that domain; a cell above 0 must link to `/debt?kind=<kind>&domain=<slug>`.
- **AUD-7** (deterministic) A cell of the heatmap must carry a level from 0 to 4: 0 for no item, else
  `ceil(4 × count / max)` where `max` is the largest count of the table; the count is written in every cell above 0,
  so the colour is never the only carrier of the figure.
- **AUD-8** (deterministic) A figure the run lacks must be replaced by a line saying it is missing and the command that
  produces it: no confidence score (AUD-1), no behaviour coverage (AUD-2), no `generate` in the usage history (AUD-4),
  no context map (AUD-5); the other figures are still shown.
- **AUD-9** (deterministic) The "Audit" group of the header of every page must link to the audit page, with a title
  saying what it holds.
- **AUD-10** (deterministic) On a repository with no run yet, the audit page must say to run `retrodoc generate`.

## Scenarios

- (AUD-1) Given a run with no confidence score, then the page says the confidence is missing and still shows the
  heatmap and the other figures (AUD-8).
- (AUD-2) Given a coverage file with entry points per feature but none per context (a `surface` run), then only the
  coverage by the use cases is shown.
- (AUD-1) Given a run with a business-language score and no confidence score, then the business language is shown
  beside the line saying the confidence is missing.
- (AUD-2) Given a saved count of a context the run no longer has, then it is not counted.
- (AUD-3) Given 1 stale section out of 300, then the page shows 1%; given 299 out of 300, 99%.
- (AUD-3) Given a use case whose feature is not in the run, then it is not counted in the total.
- (AUD-4) Given a history whose last run is a `roles`, then the last `generate` before it is shown.
- (AUD-4) Given a pass with calls whose server reported no usage, then its row says its tokens are a lower bound.
- (AUD-5) Given a context recorded done that the map no longer has, then it is not counted.
- (AUD-6) Given a debt item whose feature is not in the run (no domain), then it is in no row.
- (AUD-7) Given a table whose largest count is 8, then a cell of 1 is at level 1, of 3 at level 2, of 8 at level 4.
- (AUD-6) Given a domain slug `a&b`, then the link of its cells encodes it: `domain=a%26b`.
- (AUD-6, AUD-10) Given a domain named `<script>`, then it is shown as text (WEB-10).

## Non-goals

- No money cost: the code has no price per model; the cost is the tokens and the time the server reported.
- The progress of AUD-5 is as recorded at the end of each chain: a context whose files were edited since is still
  counted done until the next `generate` plans it again (judging it needs the roles and the file walk of `generate`).
  The page does not read the configuration either: a repository that left `domains_from = "contexts"` keeps its
  saved map, and the page still shows its progress.
- The kinds of the code no doc covers and of the project docs join the heatmap with
  `issues/web_debt_view_of_the_code.md`; the priority across the cells is `issues/debt_priority_score.md`.
- No chart library: the heatmap is an HTML table, its shades in the stylesheet (ADR 0039).
- The page calls no LLM; its figures are read at each request.
