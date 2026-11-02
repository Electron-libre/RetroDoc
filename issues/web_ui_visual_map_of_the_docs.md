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

# Milestone

Web interface for a first-time reader

# Resources

* `crates/retrodoc-web/assets/diagrams.js`, `crates/retrodoc-web/src/assets.rs`.
* ADR 0039 (server-rendered HTML, no front-end build): a new JS library is a decision for an ADR amendment.
* `docs/crates/render-mcp-cli.md`.

# Hints

* The interface is fully offline (assets embedded): no CDN.
* A large Rails run has hundreds of features: a single graph of everything will not read.

# Tracking

Plan: validated

## Understanding
Two Mermaid maps built on the server: `/map` (domains, edges = shared concepts) and `/map/<domain>` (features, human
actors, concepts), each node a link through the HTML list beside the graph. Out of scope: use case nodes, links
between use cases (not in the docs), a new JS library (so no ADR amendment).

## Measure (2026-11-02, saved runs)

| run | domains | features | use cases | max features / domain | human actors | domain pairs sharing a concept |
| --- | --- | --- | --- | --- | --- | --- |
| healthchecks | 7 | 38 | 129 | 10 | 7 | 0 |
| linkding | 9 | 32 | 81 | 9 | 2 | 17 |
| lobsters | 9 | 77 | 202 | 16 | 6 | 27 |
| second benchmark app | 8 | 98 | 392 | 32 | 6 | 19 |
| large Rails app | 7 | 85 | 762 | 33 | 6 | 21 |

- At most 9 domains and 33 features per domain: Mermaid reads (option 2); use cases (up to 372 per domain) can't be
  nodes.
- Edges between features through a shared actor form near-complete graphs (1,793 on the large app): actors are drawn
  as nodes on the domain map instead, and concepts are the links of the overview.

## Plan
1. [x] Overview map: `/map`, its graph and list, the click from a drawn node (`diagrams.js`), the map in the header
   and "Start here"; verified by `tests/map_page.rs`; makes true WEB-66, WEB-67, WEB-70, WEB-71, WEB-74, WEB-75.
2. [ ] Domain map: `/map/<domain>`, its graph and list, its 404, the link from the domain page; verified by
   `tests/map_page.rs`; makes true WEB-25, WEB-72, WEB-73, WEB-76.

## Deliverable 1: overview map

- done: `/map` draws the domains (Mermaid `flowchart TB`, edges = shared concepts, first 3 then `+n`) beside a list of
  links carrying `data-node`; `diagrams.js` opens the link of a clicked node; Map is in the header ("Read") and in
  "Start here".
- checks: `just check` ok (tests, clippy, specs); docs updated (`render-mcp-cli.md`, `ARCHITECTURE.md`, `README.md`);
  headless Chrome on a saved run of the second benchmark app: 8 nodes drawn, 8 linked, a click opens the domain link.
- red: WEB-70, WEB-71, WEB-74, WEB-75, WEB-24: `/map` answered 404 (`assertion left == right failed: left: 404, right: 200`).
- Code review: 1 findings
  - `/map/<domain>` links answer 404: the route is deliverable 2, by plan; not fixed here.
- not verified: the `LR` layout was tried and dropped for `TB` by eye (edge labels crushed the nodes).

## Decisions
- Mode: autonomous
- Base commit: 339a2260dbfc178a8e3d64625a023afeb07f45ec
- Overview edges: shared concepts only (actors are nodes of the domain maps).
