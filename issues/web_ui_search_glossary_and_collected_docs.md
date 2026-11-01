---
priority: medium
depends_on: [web_ui_server_and_domains]
why: 'search and glossary make the docs usable without knowing the tree, but the pages come first'
---
# Search the docs, read the glossary and the collected docs in the web interface

# Goal

A search box on every page of the web interface, answered by the BM25 index of the MCP server (`search_docs`,
`specs/docs-search.md`): the hits grouped by kind (domain, feature, use case, rule, concept, collected doc), each with
its confidence and a link, and the not-documented answer naming the query words no entry has. A glossary page lists
the concepts with their definition and the use cases that use them, and a collected doc of the repository is readable
as a page.

# Approach

1. A search route returning a fragment that `htmx` swaps under the box, and a full page for a direct link.
2. The glossary page from the `Glossary` artifact, sorted by name, with an anchor per concept.
3. A page per collected Markdown doc, rendered to HTML (`pulldown-cmark`, raw HTML disabled), the anchors matching
   those of the reconciliation links.
4. Router tests: a query with hits, a query with none, a glossary concept, a collected doc.

# Specs

* `specs/web-ui.md` amended; `specs/docs-search.md` is reused as is.

# Milestone

Browse the docs in a web interface

# Resources

* `crates/retrodoc-mcp/src/search.rs`, `crates/retrodoc-mcp/src/tools/search_docs.rs`
* `specs/docs-search.md`
* `crates/retrodoc-pipeline/src/glossary/`

# Hints

* The same index and relevance rule as `retrodoc search` and the MCP server: a hit list that differs between them
  would be a bug.

# Tracking

Plan: validated

## Understanding
Add to `retrodoc ui` a search page over the `search_docs` index (box on every page, `htmx` fragment), a glossary page
and a page per collected Markdown doc, so the WEB-23 links resolve. Out of scope: synonyms, a page per concept, serving
the files a collected doc links to. Spec: `specs/web-ui.md` WEB-39 to WEB-51.

## Plan
1. [x] Collected doc pages: `/docs/<path>` renders a doc `project_docs` returns with `pulldown-cmark` (raw HTML as
   text, unsafe schemes dropped, heading ids from `anchor`), served with or without a run, 404 otherwise; verified by
   router tests (~350 lines); makes true WEB-47, WEB-48, WEB-49, WEB-50, WEB-51.
2. [x] Glossary page: `/glossary` lists the merged concepts sorted by name with their definition, anchor and the use
   cases that use them, linked from every page; verified by router tests (~300 lines); makes true WEB-44, WEB-45,
   WEB-46.
3. [x] Search: the box in the layout, `/search?q=` with the hits of `GeneratedDocs::index` (limit 10, the default of
   `retrodoc search`) grouped by kind with confidence and link, the not-documented answer, a fragment for `htmx`;
   verified by router tests (~400 lines); makes true WEB-39, WEB-40, WEB-41, WEB-42, WEB-43.

## Decisions
- Mode: autonomous
- A use case uses a concept when one of its steps cites a file the concept was read from, or a business rule
  that applies to it names the concept (answer 1 a).
- At most 10 hits, the default limit of `retrodoc search` (answer 2 a).
- Base commit: 5337f41

## Deliverable 1: collected doc pages

done: `/docs/{*path}` renders a collected doc with `pulldown-cmark` (`collected.rs`, `collected.html`); `router` and
`serve` take the collected docs; `retrodoc-mcp` re-exports `ExistingDoc`.

| check | result |
|---|---|
| tests | `just check` green, `tests/doc_pages.rs` 5 tests + 1 unit |
| lint | clippy pedantic clean |
| specs | WEB-47 to WEB-51 traced |
| docs | `docs/crates/render-mcp-cli.md`, `README.md` |
| code review | see below |

red: WEB-47, WEB-48, WEB-49, WEB-50: no `<article>` in the 404 page the stub handler served (WEB-51 passed already on the fallback).

Code review: no findings (`/code-review low`).

not verified: the page in a browser (checked through the router only).

## Deliverable 2: glossary page

done: `/glossary` (`glossary.rs`, `glossary.html`) lists the merged concepts with their definition and the use cases
that use them; a `Glossary` link in the header of every page.

| check | result |
|---|---|
| tests | `just check` green, `tests/glossary_page.rs` 4 tests |
| lint | clippy pedantic clean |
| specs | WEB-44, WEB-45, WEB-46 traced |
| docs | `docs/crates/render-mcp-cli.md`, `README.md` |
| code review | see below |

red: WEB-44, WEB-45, WEB-46: `/glossary` answered 404 and the header had no `href="/glossary"`.

Code review: no findings (`/code-review low`).

not verified: the page in a browser (checked through the router only).

## Deliverable 3: search

done: a search form in the header of every page; `/search?q=` (`search.rs`, `search.html`, `search_results.html`)
groups the hits of `GeneratedDocs::index` by kind with confidence and link, or the not-documented answer; `htmx`
gets the results alone. `SEARCH_LIMIT` (10) is shared with the default `--limit` of `retrodoc search`; the rules of a
feature page carry `id="rule-<slug>"` for the hits to land on; `corpus::feature_id` is public.

| check | result |
|---|---|
| tests | `just check` green, `tests/search_page.rs` 9 tests + 1 unit |
| lint | clippy pedantic clean |
| specs | WEB-39 to WEB-43 traced, `specs/web-ui.md` accepted |
| docs | `docs/crates/render-mcp-cli.md`, `README.md` |
| code review | see below |
| real run | `retrodoc ui` on a saved linkding run: search, glossary and links checked with curl and headless Chrome |

red: WEB-39, WEB-40, WEB-41, WEB-42, WEB-43: `/search` answered 404 and no page had `<form class="search"`.
