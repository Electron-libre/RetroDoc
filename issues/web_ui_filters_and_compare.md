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
