---
priority: medium
depends_on: [web_ui_components]
why: 'the search box is the main entry of a reader, but it already works on submit'
---
# Search the docs as the user types, from the keyboard

# Goal
The search box of the header sends its query on submit and shows the hits under the header. Expected: hits while
typing (debounced), `/` to focus the box from any page, arrows and Enter to pick a hit, Escape to close, the hits as
a panel under the box with the title, the kind and the confidence of each.

# Findings
* `templates/base.html`: `form.search` with `hx-get="/search"` and `hx-target="#search-results"`, triggered on
  submit only.
* WEB-39 to WEB-43 fix the query parameter, the hits, the empty answer and the fragment answer.
* `htmx` can trigger on `keyup changed delay:300ms` and on `keyup[key=='/'] from:body`; moving through the hits with
  the arrows needs a few lines of script.

# Approach
1. `hx-trigger="input changed delay:300ms, search"` on the box, a minimum query length.
2. The hits panel component (from `web_ui_components`), with a link to the full search page.
3. Keyboard: `/` with `htmx`; arrows, Enter and Escape in a small vendored script (`assets/`, embedded, no CDN),
   with the ARIA roles of a combobox.

# Specs
* `specs/web-ui.md`: amends WEB-39 and WEB-43 (the trigger and the fragment), adds the keyboard requirements.

# Milestone
A web interface that invites to act

# Resources
* `templates/base.html`, `search.html`, `search_results.html`; `crates/retrodoc-web/src/search.rs`, `assets.rs`
* Test `crates/retrodoc-web/tests/search_page.rs`
* ADR 0039 (a small dedicated script is allowed for what `htmx` cannot do)

# Hints
* WEB-11: the script is served from `/assets/`, embedded like `diagrams.js`.
* The search index is loaded once; check the latency per keystroke on a large run before choosing the delay.
