---
priority: medium
depends_on: [web_debt_view_of_the_docs]
why: 'completes the debt view with what no doc covers and the project''s own docs; the docs side comes first'
---
# Add the code no doc covers and the contradicted project docs to the debt view

# Goal
The debt view of `web_debt_view_of_the_docs` lists the weak sections of the docs. The milestone also asks for what
the docs do not cover and where they disagree with the project's own docs. Expected: the same view lists, as items
of their own kinds, the source files no domain holds or the file budget left out, the entry points no feature holds,
the files reached by no behaviour, and the project docs the reconciliation found contradicted or missing, each with
its link (the code page, the collected doc) and filterable like the other items.

# Findings
* `DebtReport` holds `uncategorized_files`, `skipped_files`, `unplaced_entry_points`, `technical_support`,
  `entries_by_context`, `entries_by_feature` and `reconciliation` (`crates/retrodoc-pipeline/src/report/`).
* The web interface already has a page per source file (`/code/{*path}`) and per collected doc (`/docs/{*path}`).

# Approach
1. New item kinds in the debt list for the code side and the reconciliation, linked to the existing pages.
2. A large repository lists hundreds of files: candidates are grouping them by folder, or paging the list; measure
   the count on a saved run of a large repository first.

# Specs
* The spec of the debt view written by `web_debt_view_of_the_docs`.

# Milestone
Explore the documentation debt and audit it

# Resources
* `crates/retrodoc-pipeline/src/report/` (`behaviour.rs`, `reconciliation.rs`), `specs/docs-reconciliation.md`.
* `crates/retrodoc-web/src/code.rs`, `collected.rs`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
The debt page `/debt` also lists the items the project's docs contradict or leave out (reconciliation from the saved
verdicts) and the code no doc covers (entry points without a feature, files with no domain, reached by no behaviour,
not analysed), the code side grouped by folder (`specs/debt-view.md`, DBT-15 to DBT-22). Out of scope: the entry
points without a use case, the technical features, the rules set aside. Measured on a saved Lobsters run (docs
hidden): 240 files reached by no behaviour, 247 undocumented items, 2 files with no domain, 0 unplaced, 0 skipped.

## Plan
1. [ ] Read model: `GeneratedDocs::load` keeps the reconciliation (computed once, from the collected docs and the
   saved verdicts), the files with no domain, the skipped files and the behaviour coverage; `retrodoc ui` builds the
   item links from that reconciliation instead of reconciling twice; verified by internal tests; makes true nothing
   alone (prepares DBT-15 to DBT-21). ~300 lines.
2. [x] Debt items of the new kinds in `retrodoc_mcp::debt` (contradicted, undocumented, the code side by folder), the
   heatmap of the audit gains the two kinds that carry a domain (AUD-6 amended); verified by internal tests; makes
   true DBT-8, DBT-15, DBT-16, DBT-17, DBT-19, DBT-20, DBT-21, DBT-22. ~450 lines. (autonomous)
3. [x] The page: rows of the new kinds (folder items folded with their files, links to `/code/` and `/docs/`), the
   kind filter, `/code/` serving the files of the debt, English and French texts; verified by router tests and a
   check in headless Chrome; makes true DBT-18, DBT-21 (links) and the spec `accepted`. ~500 lines. (autonomous)
4. [x] Fixes of the `high` review of the issue: legacy entry point ids, the reconciliation out of `load` (ui only),
   the source access with the debt in the read model, the doc section link shared, the domain of a doc item, the
   English wording; verified by a new internal test; makes true DBT-21. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 4d88c170f5e8083b38ed2e5330fc0658e1d7501a
- 1 a: `/code/` also serves the files of the debt, in the web interface only (the MCP tools unchanged); WEB-31 and
  WEB-36 amended.
- 2 a: one item per kind and folder, its files folded under it (DBT-21).
- 3 a: the heatmap gains the columns "contradicted" and "undocumented" only; AUD-6 amended.

## Deliverable summaries

### 1. Read model
done: `GeneratedDocs` keeps `Uncovered` (the reconciliation with the collected docs, `None` without one; the files with
no domain, `DomainMap::uncategorized_paths`; the skipped files; the behaviour coverage), filled by `load`;
`item_doc_links` of `retrodoc ui` reads that reconciliation instead of reconciling again; `specs/debt-view.md`
amended (draft).

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (`debt-view` draft) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | pending |

red: none (no requirement traced: the two load tests failed to compile on the missing `uncovered`)

not verified: none. decisions needed: none.

### 2. Debt items of the new kinds
done: `DebtKind` gains six kinds; a `DebtItem` is about a `Subject` (a section, a `DocItem` of the reconciliation, a
`Folder` of code), with `domain()` and `label()`; `DebtList::no_docs`; the heatmap counts `DebtKind::WITH_DOMAIN`
(AUD-6 amended); the web names the new kinds (English, French) and lists them by label until deliverable 3.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (AUD-6 amended) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | no findings |

red: DBT-8, DBT-15, DBT-16, DBT-17, DBT-19, DBT-20, DBT-21, DBT-22: the 3 tests failed to compile on the missing
`Subject`, `DebtKind::Contradicted` and `DebtList::no_docs`; AUD-6: 8 cells where 6 were expected

Code review: no findings

not verified: none. decisions needed: none.

### 3. The page
done: the debt page shows the doc items (their kind, name, domain, link to their page, entry points, the contradicting
sections linked to `/docs/` with what each side states), the folders (their files or entry points folded, linked to
`/code/`), the line on a project with no collected doc; `/code/` and the files page serve and link the files of the
debt (WEB-31, WEB-34, WEB-36 amended); English and French texts; `specs/debt-view.md` accepted.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (`debt-view` accepted) |
| docs | `docs/crates/render-mcp-cli.md`, `README.md` |
| headless Chrome | `/debt` on a saved Lobsters run with its docs shown: 291 undocumented, 97 set apart, 12 folders reached by no behaviour (150 files in `db/old_migrations`), 1 with no domain; a debt file answers 200 on `/code/` |
| code review | no findings |

red: DBT-15, DBT-16, DBT-17, DBT-18, DBT-19, DBT-20, DBT-21, DBT-22, WEB-31, WEB-36: 5 router tests failed on the
missing rows (`assertion failed: item.contains("use case")`, `/code/lib/c.rb` answering 404)

Code review: no findings

not verified: the folding of a folder clicked in a browser (`<details>`, native); no saved run has a contradicted
item, so the `/docs/` link of a contradiction is checked by the router test only. decisions needed: none.

### 4. Fixes of the high review
done: 7 of the 10 findings of the `high` review fixed: an entry point saved before ids (`Class#method`) names no
file (`entry_file` returns `None`, no link, not served); the reconciliation moved out of `GeneratedDocs::load` into
`GeneratedDocs::reconciled`, called by `retrodoc ui` only (`retrodoc mcp` no longer pays it);
`GeneratedDocs::source_access_with_debt` holds the rule of the files served; `view::doc_section_href` shared; a doc
item whose domain has no page shows its slug; "the generated docs state"; the `Served` wrapper dropped.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md` |
| code review | 1 finding, not applicable |

red: DBT-21: written after the fix (the test asserts what the finding described: `Billing#purge` under `.`, no file).

High review, findings not fixed:
- The audit builds the folder items it does not count: a few thousand strings per request at most, small next to
  the reads of the stale check.
- The audit reads `behaviour-coverage.yaml` at each request rather than from the read model: AUD reads its figures
  at each request by its spec; the debt list reads it once, like the other pages (DBT non-goal).
- The links of the doc items parse the label of the reconciliation (`<domain>/<feature>/<slug>`): the CLI's
  `doc_links` reads it the same way, and a slug holds no `/`.

Code review: 1 finding: `retrodoc mcp` no longer reconciles at load; not applicable, no MCP tool reads the debt (the
docs say `retrodoc mcp` does not read it).

not verified: none. decisions needed: none.
