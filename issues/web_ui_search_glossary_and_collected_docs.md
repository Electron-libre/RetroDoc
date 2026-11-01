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
