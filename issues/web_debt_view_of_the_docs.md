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
