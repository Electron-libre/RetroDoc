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
2. [x] Feature page: use cases with the technical ones set apart, rules with evidence, existing docs, source files,
   entry points, stale and back-office badges; verified by router tests; makes true WEB-16, WEB-20 (feature), WEB-21,
   WEB-25 (feature under another domain).
3. [x] Use case page and diagrams: steps, narrative and its unbacked flag, business-language score, vendored
   `mermaid` loaded only there, served with a long cache; verified by router tests; makes true WEB-17, WEB-20 (use
   case), WEB-22, WEB-24, WEB-25 (use case); spec `accepted`.
4. [x] Fixes of the `high` review of the issue: rule rationale, encoded doc paths, gzip only to who accepts it,
   shared doc-link macro; verified by router tests; makes true WEB-18 (rules), WEB-23, WEB-24 (scenarios).

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

## Deliverable 2: feature page

- done: `/domains/<domain>/<feature>` served (`view::feature_page`, `feature.html`, macros `rules` and `stale`);
  `retrodoc_render::source_ref_label` replaces the two copies of the cited-lines format; the WEB-8 test no longer
  uses a feature link as the unserved path.

| check | result |
|---|---|
| tests | `just check` green, 7 new tests in `tests/item_pages.rs` |
| lint | clippy pedantic clean |
| specs | no warning on `web-ui.md` |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

- red: WEB-16, WEB-18, WEB-19, WEB-20, WEB-21: `/domains/billing/pay-invoice` answered 404 before the route
- Code review: 2 findings, none fixed: both assume two features of different domains share a slug; the pipeline makes
  feature slugs unique across the run (`features::unique_slug`), and the use cases, rules and doc links are keyed by
  the feature slug alone everywhere (Markdown pages, MCP).
- not verified: the page on a real run (layout, long lists).
- decisions needed: none.

## Deliverable 3: use case page and diagrams

- done: `/domains/<domain>/<feature>/<use case>` served (`view::use_case_page`, `use_case.html`); `mermaid` 12.2.0
  vendored gzipped (identical to the `dist/mermaid.min.js` of the npm package, whose sha512 integrity matched),
  served compressed and immutable, loaded with `diagrams.js` only by a page with a diagram; spec `accepted`.

| check | result |
|---|---|
| tests | `just check` green, 8 new tests in `tests/item_pages.rs` |
| lint | clippy pedantic clean |
| specs | `web-ui.md` accepted, every requirement traced |
| docs | `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md` |
| code review | see below |

- red: WEB-17, WEB-18, WEB-19, WEB-20, WEB-22, WEB-24: the use case pages answered 404 and `/assets/mermaid-12.2.0.min.js`
  had no route
- Code review: 1 finding, fixed: a diagram folded under "Technical details" was drawn while hidden (`startOnLoad`),
  so measured empty; `diagrams.js` now draws a folded diagram when its `<details>` first opens.
- not verified: the drawing in a browser (no browser on this machine): `mermaid` reads the escaped source through
  `innerHTML` and `entityDecode` (read in its code), and the folded case relies on the `toggle` event.
- decisions needed: none.

## Deliverable 4: fixes of the `high` review

`/code-review high 71f3f48..HEAD`: 10 findings.

| finding | outcome |
|---|---|
| rule confidence shown without its rationale (domain, feature, use case pages) | fixed: `badge_why` macro, WEB-18 test |
| doc path not percent-encoded in `/docs/` links | fixed: `view::url_path`, WEB-23 scenario and test |
| `mermaid` sent gzipped whatever `Accept-Encoding`, no `Vary` | fixed: 406 to a client that refuses gzip, `Vary: accept-encoding`, WEB-24 scenario and test |
| doc-link markup duplicated in two macros | fixed: `doc_link` macro |
| template branching on the label `"Code"` | fixed: `Evidence::code` |
| stale check ignores the files of the rule evidence | not fixed: WEB-20 judges the feature by its source files and the use case by its cited lines, as the home page and the MCP do |
| `ui` reloads the artifacts to build the `DocLinks` | not fixed: one more parse at start; reconciling from `GeneratedDocs` needs a `DomainMap` it does not keep |
| business language flagged under 0.5 while set apart under 0.8 | not fixed: the Markdown page flags it the same way (`score_line`); the two must agree |
| `DomainRule` close to `RuleView` | not fixed: a domain rule shows its feature, not its evidence, like the Markdown page |
| quadratic dedup of the cited paths | not fixed: a use case cites a few files |

| check | result |
|---|---|
| tests | `just check` green, 3 new tests |
| lint | clippy pedantic clean |
| specs | 2 scenarios added to `web-ui.md` |
| docs | `docs/crates/render-mcp-cli.md` |

- red: WEB-18, WEB-23, WEB-24: `"backed by code alone" missing`, the raw `docs/C# guide?.md` href, 200 instead of 406
- Code review (`low` of this deliverable): 1 finding, fixed: `gzip;q=0, *` was taken as accepting gzip; a named
  coding now overrides `*`.
- not verified: the pages in a browser.
- decisions needed: none.
