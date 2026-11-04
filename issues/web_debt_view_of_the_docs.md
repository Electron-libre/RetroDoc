---
priority: high
why: 'the view the milestone is built on: the dashboard and the priority score read the same debt items'
---
# Show the debt of the docs sections in one filterable view of the web interface

# Goal
The debt of the docs is only in the debt report (`retrodoc report`, `docs/_retrodoc/coverage-report.md`), one long
page the web interface does not show. Expected: a "Debt" page of `retrodoc ui` listing, as one list, the sections of
the docs that need a human: weak and unscored use cases and features, unbacked narratives, use cases that read as
technical or are set apart, and sections whose cited files changed since the run (stale). Each item names its kind,
its domain, its score or reason, and links to its page; the list filters by kind and by domain.

# Findings
* `DebtReport` (`crates/retrodoc-pipeline/src/report/mod.rs`) already holds `weak_sections`, `unscored_use_cases`,
  `unbacked_narratives`, `technical_use_cases`, `set_apart_use_cases` (a count only), built with no LLM call from
  `.retrodoc/cache/`.
* The report is assembled in the CLI (`crates/retrodoc-cli/src/commands/report.rs`: `build_report`, then the scope,
  behaviour coverage, technical features, reconciliation and set-aside rules added one by one): the web crate cannot
  reuse it as it is.
* Freshness is computed in `retrodoc-mcp` (`freshness.rs`, `Freshness::stale`), and the web interface already shows
  the stale files of a code page (`retrodoc-web/src/view.rs`, `stale_files`); no page lists the stale sections.

# Approach
1. Move the assembly of the debt report out of the CLI (a loader of `DebtReport` from a repo root) so `report` and
   `ui` read the same report.
2. A typed list of debt items (kind, domain, label, score or reason, link) built from the report and the freshness,
   in the read model, independent of HTTP (ADR 0039).
3. The page `/debt`, its link in the navigation, filters by kind and domain as query parameters swapped by `htmx`.
4. Candidates for the set apart use cases: list them (the report only counts them) or show the count with a link to
   their features.

# Specs
* `specs/web-ui.md` (new requirements for the debt page), or a new spec `debt-view`.

# Milestone
Explore the documentation debt and audit it

# Resources
* `crates/retrodoc-pipeline/src/report/`, `crates/retrodoc-cli/src/commands/report.rs`.
* `crates/retrodoc-web/src/` (`lib.rs` routes, `view.rs`, `pages.rs`), `crates/retrodoc-mcp/src/freshness.rs`.
* `docs/adr/0039-a-server-rendered-web-interface-over-the-generated-docs.md`, `specs/web-ui.md`,
  `docs/crates/render-mcp-cli.md`.

# Hints
* The interface calls no LLM and writes nothing (ADR 0039); the page must work on a run with no confidence scores.
* Check the page in headless Chrome, not only through the HTML of the tests.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
A page `/debt` of `retrodoc ui` listing the docs sections that need a human (low confidence, not scored, unbacked
narrative, technical language, set apart, stale), filterable by kind and domain (`specs/debt-view.md`). Out of scope:
the code side and the reconciliation (`web_debt_view_of_the_code`), the priority. Means differ: the read model calls
`build_report` (what `retrodoc report` uses for these sections); moving the full assembly out of the CLI (scope,
coverage, reconciliation, which need the config and the docs) goes to `web_debt_view_of_the_code`, the first to need it.

## Plan
1. [x] Report sections carry their slugs: `WeakSection` and the unscored use cases name their domain, feature and
   use case, and the report lists the set apart use cases with their reason (the Markdown unchanged); verified by
   the report tests; makes true nothing alone (prepares DBT-2 to DBT-6). ~250 lines. (autonomous)
2. [x] Debt items in the read model: `retrodoc_mcp::debt` builds the typed items (kind, domain, section, score or
   reason) from the report and the freshness, independent of HTTP; verified by internal tests; makes true DBT-2 to
   DBT-8. ~400 lines. (autonomous)
3. [x] The page `/debt`: route, template, filters swapped by `htmx`, header link and "Start here" line, English and
   French texts; verified by router tests and a check in headless Chrome; makes true DBT-1, DBT-9 to DBT-14. ~600 lines. (autonomous)
4. [x] Fixes of the `high` review of the issue: the "Start here" count of the debt judged at each request, each cited
   file hashed once per call, the misleading "no section needs a human" on a run with no score (DBT-11 amended), the
   kind labels by `match`, `find_feature` and `StaleFile::of` shared, the sections sorted without allocating; verified
   by two new router tests; makes true DBT-11, DBT-13. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 8aa4143ef0165d729643c8692a4bdbb124a2bb73
- 1 a: a run with no confidence score shows one line and lists no "not scored" item.
- 2 a: each set apart use case is an item with its reasons.
- 3 a: a new "Audit" group in the header, and a "Start here" line with the number of items; WEB-66, WEB-67 and the
  non-goal of `specs/web-ui.md` on the debt view are amended.

## Deliverable summaries

### 1. Report sections carry their slugs
done: `Section` (domain, feature, use case) on `WeakSection` and the unscored use cases, `set_apart_use_cases` lists
each use case with its `SetApartReason`s (`UseCase::set_apart_reasons` in core, `is_technical` built on it); the
Markdown of the report unchanged.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass |
| docs | no change needed (internal API) |
| code review | no findings |

red: none (no requirement traced yet; the tests failed to compile on the missing `Section` and `SetApartReason`)

Code review: no findings

not verified: none. decisions needed: none.

### 2. Debt items in the read model
done: `retrodoc_mcp::debt::debt` lists the `DebtItem`s (kind, section, score, rationale, set apart reasons, changed
files) from `build_report` and the freshness; `use_case_cited_paths` shared with the use case page; DBT-8 amended to
the order of the report.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | no findings |

red: DBT-2, DBT-3, DBT-4, DBT-5, DBT-6, DBT-7, DBT-8: 5 tests failed on the empty list of the stub
(`assertion left == right failed`)

Code review: no findings

not verified: none. decisions needed: none.

### 3. The debt page
done: `/debt` (`debt.rs`, `debt.html`, `debt_list.html`), filters by kind and domain swapped by `htmx`, the "Audit"
group of the header and the "Start here" line with the count of items, English and French texts; `fragment_or_page`
takes the page built (the debt is built off the async workers); WEB-66, WEB-67 and a non-goal of `specs/web-ui.md`
amended; `specs/debt-view.md` accepted.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (`debt-view` accepted) |
| docs | `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md`, `README.md` |
| headless Chrome | `/debt` and `/` on a saved run: 73 items, the Audit link, the "Start here" line |
| code review | no findings |

red: DBT-1, DBT-3, DBT-9, DBT-10, DBT-11, DBT-12, DBT-13, DBT-14, WEB-77: 9 tests failed on `/debt` answering 404
(`assertion left == right failed: left: 404`)

Code review: no findings

not verified: the `htmx` swap of the filters clicked in a browser (the fragment is tested through the router). decisions needed: none.

### 4. Fixes of the high review
done: 6 of the 7 findings of the `high` review fixed (see the plan line).

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (DBT-11 amended) |
| docs | none beyond the doc comments |
| code review | no findings |

red: DBT-3, DBT-11, DBT-13: written after the fix (the two tests assert what the findings described: a count of 5 left
after a file deleted, the "needs a human" line next to the no-score note).

High review, finding not fixed: the debt report rebuilt at each request rather than once at start. It reads the
in-memory docs loaded once, so what it shows is the same; its cost (a sort over the use cases) is small next to the
reads of the stale check.

Code review: no findings

not verified: none. decisions needed: none.
