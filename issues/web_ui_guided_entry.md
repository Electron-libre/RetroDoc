---
priority: medium
depends_on: [web_ui_application_name]
why: 'a first-time reader does not know where to start; the home page is the entry of every other view, and it shows the application name of the other issue'
---
# Give the web interface a home page that tells where to start

# Goal

A reader who opens the interface for the first time knows what the application does, what the docs cover and where
to start reading. Today the home page is a flat list of domains and features, beside a tree that repeats it, and the
header lists Files, Glossary, Rules, Actors with no hint of what each holds or in which order to read them.

# Findings

* 2026-11-02, user feedback on a saved run of a public Django app (healthchecks): "the navigation is not that
  intuitive; one does not know where to start".
* The home page (`pages.rs`, `home`) shows the domain rows only; the product brief (purpose, users, capabilities,
  external systems, open questions) is in `.retrodoc/cache/product.yaml` and is not shown anywhere in the interface.
* The run has 6 domains and dozens of features: the home list is long before the first use case.

# Approach

Ask the user which reading paths matter first, then candidates (can be combined):

1. A home page in sections: what the application does (the purpose and users of the brief), its main domains as
   cards with one line and their counts, then "start here" links (the actors, the glossary, the debt).
2. A short line under each header link saying what the page holds, or the links grouped (Read: domains, actors;
   Look up: glossary, rules, files).
3. Breadcrumbs on the domain, feature and use case pages, and "next / previous" between the use cases of a feature.
4. Measure with a reader: a walk-through on the healthchecks run before and after, with headless Chrome screenshots.

# Specs

* `specs/web-ui.md` amended (the home page requirements).

# Milestone

Web interface for a first-time reader

# Resources

* `crates/retrodoc-web/src/pages.rs`, `templates/home.html`, `templates/base.html`.
* `crates/retrodoc-mcp` read model (what the brief exposes).
* ADR 0039; `PRODUCT.md` (the readers of the docs and what they need).

# Hints

* The "Explore the documentation debt" milestone plans a debt view and a dashboard: the home page should leave them
  a place rather than build them.

# Tracking

Plan: validated

## Understanding
A first-time reader lands on a home page that says what the application does and who uses it, points to where to
start, then lists the domains; the header says what each page holds; a use case links to its neighbours. Breadcrumbs
already exist (WEB-19). Out of scope: the debt view and dashboard (their own milestone), the visual map (its issue).

## Plan
1. [x] Home page in sections: brief users, "Start here" links with a line and a count each, domains as cards;
   verified by `tests/pages.rs` page tests; makes true WEB-65, WEB-66.
2. [ ] Header links grouped "Read" / "Look up" with titles; verified by a page test on every route; makes true WEB-67.
3. [ ] Previous / next links between the use cases of a feature; verified by `tests/item_pages.rs`; makes true WEB-68.
4. [ ] Walk-through on the saved healthchecks run, headless Chrome screenshots before and after (no LLM call),
   findings in this issue; verified by the screenshots.

## Decisions
- Mode: autonomous
- Base commit: af0a1678fac753cd9f77c17ea53fc00fbe1ad6da
- Reading path (2026-11-02): purpose, users, "Start here", then the domains (WEB-65).
- The home page shows neither the external systems nor the open questions of the brief.
- Header grouped "Read" / "Look up" with a title per link; previous / next between use cases kept.

## Deliverable 1

- done: the home page shows the purpose, the users of the brief, a "Start here" section (actors, glossary, rules,
  files, each with what it holds and its count) and the domains as cards; `retrodoc ui` passes the brief's users.
- red: WEB-65, WEB-66: `no Buyers who order goods in <main>`, `no id="start-here" in` the home page.
- Before / after: `~/retrodoc-runs/guided-entry-shots/before-home.png`, `d1-home.png` (healthchecks run).

