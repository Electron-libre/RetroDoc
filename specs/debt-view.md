---
id: DBT
status: draft
adrs: [0039]
---

# Debt view of the docs

## Intent

The debt page of `retrodoc ui`, `/debt`, lists as one list the sections of the docs of the last run that need a
human, so whoever audits the docs sees what to check and filters it by kind and domain, instead of reading the debt
report of `retrodoc report` from top to bottom.

## Requirements

- **DBT-1** (deterministic) The debt page, `/debt`, must list the debt items of the last run, each with its kind,
  the name of its domain, its section (the use case with its feature, or the feature), its score or its reason, and a
  link to the page of its section.
- **DBT-2** (deterministic) An item of the kind "low confidence" must be listed for each use case whose confidence is
  below `LOW_CONFIDENCE_THRESHOLD`, and each feature below it that has no use case, with its score and its
  rationale: the sections the low-confidence section of `retrodoc report` lists.
- **DBT-3** (deterministic) An item of the kind "not scored" must be listed, when at least one use case of the run
  has a confidence, for each use case with none; when none has one, the page says the run has no confidence
  score and lists no item of that kind.
- **DBT-4** (deterministic) An item of the kind "unbacked narrative" must be listed for each use case whose narrative
  the confidence pass judged unsupported by the cited code (`narrative_check` at 0), with its rationale.
- **DBT-5** (deterministic) An item of the kind "technical language" must be listed for each use case whose
  business-language score is below `LOW_CONFIDENCE_THRESHOLD`, with its score and its rationale.
- **DBT-6** (deterministic) An item of the kind "set apart" must be listed for each use case the docs set apart
  under "Administration and technical" (`UseCase::is_technical`), with its reason: its back-office feature, its
  business language or its confidence.
- **DBT-7** (deterministic) An item of the kind "stale" must be listed for each feature and each use case flagged
  stale on its page (WEB-20), with the files that changed.
- **DBT-8** (deterministic) The items must be ordered by kind in the order of DBT-2 to DBT-7, and within a kind as
  `retrodoc report` lists them (the lowest score first, then by section; the unbacked narratives by section), the not
  scored, set apart and stale ones in the order of the run, a stale feature before its use cases.
- **DBT-9** (deterministic) The debt page must keep only the items of the kind `?kind=<kind>` and of the domain
  `?domain=<slug>`, each given, both when both are; an empty value filters nothing.
- **DBT-10** (deterministic) The debt page must show a form whose choices are the kinds and the domains of the run,
  the ones of the query selected, that asks the page again with them.
- **DBT-11** (deterministic) The debt page must say no section needs a human when the run has no item, and that no
  item matches when the filters keep none.
- **DBT-12** (deterministic) A debt request must be answered, with the header `HX-Request` and without
  `HX-History-Restore-Request`, with the list of the items alone, without the layout of the page; any other with
  the full page.
- **DBT-13** (deterministic) Every page must link to the debt page under an "Audit" group of its header, and the
  "Start here" section of the home page must link to it with its number of items.
- **DBT-14** (deterministic) On a repository with no run yet, the debt page must say to run `retrodoc generate`.

## Scenarios

- (DBT-1) Given a use case whose feature is not in the run (artifacts of an older run), then it is listed without a
  link and no `?domain=` keeps it.
- (DBT-2) Given a feature below the threshold that has use cases, then it is not listed: its use cases explain it.
- (DBT-4) Given a narrative judged partly backed (`narrative_check` above 0), then it is not listed.
- (DBT-6) Given a use case set apart for two reasons, then it is one item naming both.
- (DBT-7) Given a cited file with no recorded hash (left out by the file budget), then no section citing it is stale.
- (DBT-2, DBT-5) Given a use case both low in confidence and in business language, then it is listed under both kinds.
- (DBT-9) Given a filter whose value names no kind or domain of the run, then no item matches.
- (DBT-10, DBT-1) Given a section name `<script>`, then it is shown as text (WEB-10).

## Non-goals

- The code no doc covers and the project docs contradicted or missing are not items here: they come with
  `issues/web_debt_view_of_the_code.md`.
- No priority across the kinds: the order is fixed until `issues/debt_priority_score.md` ranks the items.
- The page does not replace `retrodoc report` nor `coverage-report.md`, and calls no LLM (ADR 0039).
- The stale items are judged when the page is asked, the other items read once at start, like the other pages.
