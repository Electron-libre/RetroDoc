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
2. [x] Header links grouped "Read" / "Look up" with titles; verified by a page test on every route; makes true WEB-67.
3. [x] Previous / next links between the use cases of a feature; verified by `tests/item_pages.rs`; makes true WEB-68.
4. [x] Walk-through on the saved healthchecks run, headless Chrome screenshots before and after (no LLM call),
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

## Deliverable 2

- done: the header groups its links under "Read" (actors) and "Look up" (glossary, rules, files), each link titled
  with what its page holds (the hints of the home page, `view::REFERENCE_PAGES`).
- red: WEB-67: `no aria-label="Read" in <header class="top">`.
- Screenshot: `~/retrodoc-runs/guided-entry-shots/d2-header.png`.

## Deliverable 3

- done: a use case page ends on links to the previous and the next use case of its feature, in the order of the run;
  `specs/web-ui.md` back to `accepted` (every requirement traced).
- red: WEB-68: `"href=\"/domains/billing/pay-invoice/retry-job\"" missing` (no sequence links on the page).

## Deliverable 4: walk-through on the saved healthchecks run

Screenshots in `~/retrodoc-runs/guided-entry-shots/` (outside git), no LLM call.

| Step | Before (`before-*.png`) | After (`after-*.png`) |
|---|---|---|
| Home | name, purpose, a table of 6 domains; nothing says what Files, Glossary, Rules, Actors hold | purpose, 4 users of the brief, "Start here" with what each page holds and its count (11 actors, 19 concepts, 346 rules, 50 cited files), 6 domain cards |
| Header | Files, Glossary, Rules, Actors flat | "Read: Actors", "Look up: Glossary, Rules, Files", each titled |
| Use case | breadcrumb only; back to the tree to read the next one | a "Next" link to the next use case of the feature at its foot |

Found: the root of the breadcrumb was still labelled "Domains" while the page it opens is now headed by the
application and titled "Home": relabelled "Home" in deliverable 5.

## Deliverable 5: docs and review of the whole issue

- done: `update-docs` over `af0a167..HEAD` (README line of `ui`); the root of the breadcrumb relabelled "Home".
- red: none (a relabel; the WEB-19 test now asserts the label).
- Code review (high, whole issue): 7 findings.
  - fixed: counts of 1 in the singular (`1 feature`, `1 rule`); the actors count is a row per actor and kind, as on
    the actors page; the previous / next links built by `use_case_href`; the header groups read from a `group` of each
    page of reference, not from its position; the counts of "Start here" computed once in `Site::new`, with no dead
    "no run" fallback.
  - not fixed: "a feature slug shared by two domains mixes their use cases": the features pass makes slugs unique
    across domains (`features/from_surface.rs`), and a use case names its feature by slug alone.

