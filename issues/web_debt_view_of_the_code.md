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
