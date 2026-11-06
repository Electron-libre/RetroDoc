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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Search as the user types (300 ms after the last change, from 2 characters, the server answering an empty panel
below that), the hits as a combobox panel under the box (title, kind, confidence, link to the full page), `/` to
focus the box, arrows/Enter/Escape in a small embedded script. Out of scope: matching a word being typed (the index
stays that of `retrodoc search`). The minimum length is enforced by the server, so the trigger stays plain `htmx`.

## Plan
1. [x] Live panel: `hx-trigger="input changed delay:300ms, search"` and `hx-sync` on the box, the panel template
   (`search_panel.html`) targeted inside the search form, the full page showing its results in its content, the
   panel styles; latency per keystroke measured on the largest run of the runs directory; verified by
   `tests/search_page.rs`; makes true WEB-39, WEB-43, WEB-109; ~350 lines. (autonomous)
2. [x] Keyboard: `assets/search.js` (embedded, served from `/assets/`), the combobox attributes of WEB-110, `/`,
   arrows, Enter, Escape, click outside; verified by `tests/search_page.rs` reading the served markup and script
   (internal: the keys run in the browser) and a check by hand in headless Chrome; makes true WEB-110 to WEB-112;
   ~300 lines. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: a4fa88671d55caa9e58718779490cec5ea79e320
- Minimum query length: 2 characters once trimmed (1 a); the panel is a flat list in the order of WEB-40, the kind on each row (2 a).

## Deliverable 1: live panel
- done: the box sends itself as it is typed (`hx-trigger`, `hx-sync`), `search_panel.html` swapped into
  `#search-panel` under the box, an empty answer under 2 characters, the search page shows its results in its
  content, the panel styles; docs `docs/crates/render-mcp-cli.md`, `docs/dev/web-ui.md`.
- checks: project-check green; size 308 lines; panel checked in headless Chrome at 1440 and 390 px
  (`screenshots/live-search` of the runs directory).
- Latency: < 1 ms per `htmx` search on the largest saved run (about 4 MB of artifacts); the 300 ms delay only
  spares the requests of a word being typed.
- red: WEB-39, WEB-43, WEB-109: the box had no `hx-trigger` (`hx-get="/search"` missing on the input), the fragment
  was `<section class="results"`, no hit row with its kind, no empty answer for `i`.
- Code review: 1 finding. Fixed: a stale doc comment of `no_run_results` left above the new one.
- not verified: none. decisions needed: none.

## Deliverable 2: keyboard
- done: the box a combobox over the panel (`role`, `aria-*`, an `id` per option), `assets/search.js` embedded and
  loaded by every page (`/`, arrows, Enter, Escape, click outside), the active hit lit; spec `accepted` (WEB-109 to
  WEB-112 traced); docs `docs/crates/render-mcp-cli.md`, `docs/dev/web-ui.md`.
- checks: project-check green; checked by hand in headless Chrome on the English `delivery_router` run: `/` focuses
  and selects (and is typed in a field), Down/Up wrap through the 10 hits and the link to the full page, Escape and
  a click outside close, Down opens again, Enter follows the active hit, Enter with none opens `/search?q=order`.
- red: WEB-110, WEB-111, WEB-112: no `role="combobox"` on the box, no `/assets/search.js` in the pages.
- Code review: no findings
- not verified: a screen reader (the ARIA markup is tested, not read aloud). decisions needed: none.
