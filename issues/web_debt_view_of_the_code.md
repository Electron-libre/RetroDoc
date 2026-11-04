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
2. [ ] Debt items of the new kinds in `retrodoc_mcp::debt` (contradicted, undocumented, the code side by folder), the
   heatmap of the audit gains the two kinds that carry a domain (AUD-6 amended); verified by internal tests; makes
   true DBT-8, DBT-15, DBT-16, DBT-17, DBT-19, DBT-20, DBT-21, DBT-22. ~450 lines.
3. [ ] The page: rows of the new kinds (folder items folded with their files, links to `/code/` and `/docs/`), the
   kind filter, `/code/` serving the files of the debt, English and French texts; verified by router tests and a
   check in headless Chrome; makes true DBT-18, DBT-21 (links) and the spec `accepted`. ~500 lines.

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
