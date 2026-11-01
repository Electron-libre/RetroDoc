---
priority: high
depends_on: [web_ui_server_and_domains]
why: 'the body of the docs: without these pages the interface shows only the list of domains'
---
# Show the domain, feature and use case pages in the web interface

# Goal

Render in the web interface what the Markdown pages of `docs/functional/` hold, in a structured form: a domain page
with its features (the back-office ones set apart), a feature page with its description, entry points, rules and use
cases (the technical ones under "Administration and technical"), and a use case page with its narrative, primary
actor, steps (actor, action, detail, cited lines), the Mermaid diagram drawn in the browser, the business language and
confidence scores with their rationale, and the links to the existing docs that document or contradict it.

# Approach

1. One template per page, from the read model of `web_ui_server_and_domains`; a breadcrumb domain, feature, use case.
2. The vendored `mermaid` script, embedded like `htmx`, draws `diagram_mermaid`; the source of the diagram stays
   readable when the script fails.
3. The badges: confidence (flagged below `LOW_CONFIDENCE_THRESHOLD`), stale when a cited file changed since the run
   (`Freshness`), set apart (`UseCase::is_technical`, `Feature::back_office`), unbacked narrative (`narrative_check`).
4. The existing docs of the reconciliation (`DocLinks`) as links, `states otherwise` shown for a contradiction.
5. Router tests per page, an unknown slug answered by the 404 page.

# Specs

* `specs/web-ui.md`: the requirements of the three pages, amended at `issue-workflow` step 1.

# Milestone

Browse the docs in a web interface

# Resources

* `crates/retrodoc-render/src/markdown/` (`domain.rs`, `feature.rs`, `use_case.rs`): what each page shows today
* `crates/retrodoc-render/src/doc_links.rs`, `specs/docs-reconciliation.md`
* `crates/retrodoc-core/src/model.rs` (`UseCase::is_technical`, `Step::label`, the thresholds)
* `issues/done/technical_and_back_office_use_cases.md`

# Hints

* Show the same things as the Markdown pages, in the same order of sections, so the two never disagree; reuse the
  helpers of `retrodoc-render` rather than copying their rules.
* The `mermaid` script is large: serve it with a long cache header and load it only on the pages that draw a diagram.

# Tracking

Plan: validated

## Understanding
Serve `/domains/<d>`, `/domains/<d>/<f>`, `/domains/<d>/<f>/<u>` with what the Markdown pages show, in the same order,
plus badges (low, stale, set apart, unbacked), a breadcrumb and the Mermaid diagram drawn in the browser. Out of
scope: serving the collected docs (the links of WEB-23 lead to `/docs/...`, served by
`web_ui_search_glossary_and_collected_docs`), the cited code, search and glossary.
Means: `retrodoc-web` depends on `retrodoc-render` (ADR 0039) for `DocLinks`, the anchor of a heading, the rule kind
label and the "Administration and technical" heading, made public there; `retrodoc ui` builds the `DocLinks` from the
saved reconciliation judgements like `render` does (no LLM call) and hands them to the router.

## Plan
1. [x] Domain page and doc links: render helpers made public, `DocLinks` from `ui` into the router, breadcrumb, domain
   template, 404 on unknown slugs; verified by router tests; makes true WEB-15, WEB-18 (domain), WEB-19, WEB-23,
   WEB-25 (domain).
2. [ ] Feature page: use cases with the technical ones set apart, rules with evidence, existing docs, source files,
   entry points, stale and back-office badges; verified by router tests; makes true WEB-16, WEB-20 (feature), WEB-21,
   WEB-25 (feature under another domain).
3. [ ] Use case page and diagrams: steps, narrative and its unbacked flag, business-language score, vendored
   `mermaid` loaded only there, served with a long cache; verified by router tests; makes true WEB-17, WEB-20 (use
   case), WEB-22, WEB-24, WEB-25 (use case); spec `accepted`.

## Decisions
- Mode: autonomous
- Base commit: 71f3f48719e45c9c6469bfd1112ff3366fa5835c
- `mermaid` 12.2.0 vendored gzip-compressed (`mermaid-12.2.0.min.js.gz`, served with `Content-Encoding: gzip`): the
  plain file is 5.5 MB and 4,330 lines.
- The existing doc links point to `/docs/<path>#<anchor>`, served by `web_ui_search_glossary_and_collected_docs` (404
  until then).
- The feature page also shows the entry points (the issue asks for them; the Markdown page lists only the source
  files): shown last, so the order of the shared sections stays the Markdown one.
- Unbacked narrative: `narrative_check` at 0, the rule of the debt report (`unbacked_narratives`).

## Deliverable 1: domain page and doc links

- done: `/domains/<domain>` served (`view::domain_page`, `domain.html`, shared macros in `macros.html`);
  `retrodoc-web` depends on `retrodoc-render`, whose `anchor`, `kind_label`, `TECHNICAL_HEADING`/`NOTE` and
  `DocLinks::of` are public; `retrodoc ui` builds the `DocLinks` (`commands::docs::item_doc_links`).

| check | result |
|---|---|
| tests | `just check` green, 5 new tests in `tests/item_pages.rs` |
| lint | clippy pedantic clean |
| specs | `just check-specs`: no warning on `web-ui.md` |
| docs | CLAUDE.md, ARCHITECTURE.md, `docs/crates/render-mcp-cli.md` |
| code review | see below |

- red: WEB-15, WEB-18, WEB-19, WEB-23: `assertion left == right failed: /domains/billing` (404 before the route)
