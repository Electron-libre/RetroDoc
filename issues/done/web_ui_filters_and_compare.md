---
priority: medium
depends_on: [web_ui_components]
why: 'filters work but read as a raw form; a comfort over the forms that write'
---
# Make the filters of the debt and the rules and the run comparison direct to use

# Goal
The debt and rules pages filter through a row of selects plus a "Filter" button, though the list already updates on
change; the history page compares two runs through two selects of dates and a "Show" button. Expected: the filters
read as chips or segmented choices, apply on change with no button when `htmx` runs, show how many are active and
offer to reset them; the comparison picks its two runs as directly.

# Findings
* `templates/debt.html` and `rules.html`: a `form.filters` with `hx-trigger="change"` and `hx-push-url`, plus a
  submit button kept for the no-JS path.
* `templates/history.html`: `form.compare` with two `select` of finish dates and a submit button, no `htmx`.
* WEB-55 and WEB-56 fix the choices and the fragment answer of the rules filters.

# Approach
1. Filter component: chips (radio or checkbox inputs styled) or selects restyled, the active ones highlighted, a
   count and a reset link; the submit button hidden when `htmx` is loaded, kept in `<noscript>` or the no-JS path.
2. History: the two runs as a list with "from"/"to" choices or a swap button, updated with `htmx`.

# Specs
* `specs/web-ui.md`: amends WEB-55 (the form of the rules) and the matching requirements of the debt and history
  pages if their form changes.

# Milestone
A web interface that invites to act

# Resources
* `templates/debt.html`, `debt_list.html`, `rules.html`, `rules_list.html`, `history.html`
* `crates/retrodoc-web/src/debt.rs`, `rules.rs`, `history.rs`; tests `debt_page.rs`, `rules_page.rs`,
  `history_page.rs`

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
The debt and rules filters become a kind chip group (radios) plus restyled selects for the domains and features (too
many for chips), applied on change with the submit button left to `<noscript>`, the active ones lit, an active count
and a reset link in the list (so the `htmx` fragment updates them). The history runs list becomes the comparison form
itself (a `from` and a `to` radio per run, swapped with `htmx` on change) plus a swap link. No new filter, no
free-text filter.

## Plan
1. [x] Filters of the debt and rules pages: kind chips, selects, `<noscript>` button, active count and reset link in
   the list, CSS; verified by tests in `debt_page.rs` and `rules_page.rs` and before/after screenshots; makes true
   WEB-107, WEB-108, WEB-55, DBT-10. (autonomous)
2. [x] Run comparison of the history page: runs table with `from`/`to` radios, `htmx` swap on change, swap link;
   verified by tests in `history_page.rs` and screenshots; makes true HPG-5, HPG-16. (autonomous)
3. [x] Docs of the whole issue (`docs/dev/web-ui.md` components) and the fixes of the `high` review. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: abe9792
- Chips for the kinds only; the domains and features stay restyled selects (their number grows with the run).
- The active count and the reset link sit at the top of the list, so the `htmx` fragment refreshes them.

## Deliverable 1: filters of the debt and rules pages
- done: the kinds as radio chips ("All" first), the domains and features as selects lit when off "All", the submit
  button in `<noscript>`, the count of the active filters and a reset link atop the list (page and fragment); the
  labels top-aligned and the selects held within a narrow screen.
- checks: tests, lint, specs (`cardinal run project-check`), docs (`docs/dev/web-ui.md`, `docs/crates/render-mcp-cli.md`);
  shots `screenshots/before-abe9792` and `screenshots/after-filters` (plus `rules-filtered-*`) of the runs directory,
  served from a copy of a linkding run.
- red: WEB-107, WEB-108, WEB-55, DBT-10: no `<input type="radio" name="kind" …>` in the form, no "active filter" in
  the list.
- not verified: the change of a chip in a real browser click (the radios and `hx-trigger="change"` only checked in
  the markup); the keyboard focus ring of a chip seen only in the CSS.
- Code review: no findings.
- decisions needed: none.

## Deliverable 2: run comparison of the history page
- done: with two runs or more the runs table is the comparison form (a `from` and a `to` radio per run, the compared
  rows lit), sent on change and swapped into `#history-body` with `hx-select`, the button in `<noscript>`, a link
  that swaps the runs; a long history scrolls within the table. The three specs back to `accepted`.
- checks: tests, lint, specs (`cardinal run project-check`), docs (`docs/dev/web-ui.md`, `docs/crates/render-mcp-cli.md`);
  shots `history-compare-*` in `screenshots/before-abe9792` and `screenshots/after-filters`, the scratch copy seeded
  with the three snapshots of the tests.
- red: HPG-5, HPG-16: the form had no `hx-get`, the runs no radio, no swap link.
- not verified: a click on a radio or the swap link in a real browser (the `htmx` attributes checked in the markup).
- Code review: no findings.
- decisions needed: none.

## Deliverable 3: docs of the whole issue and the fixes of the `high` review
- done: an `htmx` history request answered with `history_body.html` alone (HPG-5 amended); an id on each radio so
  `htmx` gives the focus back after a swap; the indicator named by id, so the swap link shows it too; the names of
  the swap link percent-encoded (`audit::query_encoded`, HPG-16 amended); the radio style scoped to `table.runs`
  (it shrank the hidden radio of a chip); the heads of the runs table sticky; `Filter`, `Choice` and
  `active_filters` moved to `view.rs`, the count of the active filters a method of the page rather than a field;
  the states table of `docs/dev/web-ui.md` and a map row for the look of the web interface in
  `.claude/update-docs-map.md`.
- checks: tests, lint, specs (`cardinal run project-check`), docs; shots retaken in `screenshots/after-filters`.
- red: HPG-5, HPG-16: the `htmx` history request got the full page, the radios had no id, the swap link no
  indicator and its names raw.
- Code review (`high`): 9 findings; 8 fixed as above; not fixed: a value that names no choice still counts as an
  active filter, as WEB-108 says (the list shows no item and the reset link clears it).
- not verified: the focus kept after a swap and a real click on a radio, in a browser (ids and attributes checked
  in the markup).
- Code review (`low`): no findings.
- decisions needed: none.
