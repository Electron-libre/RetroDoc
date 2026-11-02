---
priority: low
depends_on: [web_ui_guided_entry]
why: 'a second way into the docs beside the home page and the tree: a comfort, to place once the home page is settled'
---
# Explore the documentation through a visual map in the web interface

# Goal

A page of the web interface draws the documentation as a map or graph: the domains, their features and use cases,
and the links between them (actors shared by several features, use cases that call one another, entities used across
domains), each node a link to its page. A reader sees the shape of the application at a glance and moves through it
by clicking.

# Findings

* 2026-11-02, user feedback: "some kind of map or graph to explore the documentation visually".
* The interface already ships Mermaid (`assets/mermaid-12.2.0.min.js.gz`) for the diagrams of the use cases; no page
  draws the docs as a whole.
* The read model holds what such a map needs: the tree of domains, features and use cases, the actors of the steps,
  the glossary entities and the files cited.

# Approach

Decide first what the map shows (one view or several), then how it is drawn:

1. A tree map (domains as areas, features inside, sized by use case count), static SVG rendered on the server.
2. A graph drawn by Mermaid from a source built on the server (domains and features as nodes, actors and shared
   entities as links); readable up to a few dozen nodes, so one graph per domain plus an overview of the domains.
3. A force-directed graph from a small JS library embedded like the others, for larger runs.

Measure the size first: nodes and edges per view on the saved runs (healthchecks, linkding, lobsters, a large Rails
app) decides between 2 and 3.

# Specs

* `specs/web-ui.md` amended (the map page).

# Resources

* `crates/retrodoc-web/assets/diagrams.js`, `crates/retrodoc-web/src/assets.rs`.
* ADR 0039 (server-rendered HTML, no front-end build): a new JS library is a decision for an ADR amendment.
* `docs/crates/render-mcp-cli.md`.

# Hints

* The interface is fully offline (assets embedded): no CDN.
* A large Rails run has hundreds of features: a single graph of everything will not read.
