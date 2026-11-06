# The look of the web interface

How to check a change of the look of `retrodoc ui` (`crates/retrodoc-web`), and the states each form must show.
The behaviour of the pages is in `specs/web-ui.md` and `specs/web-ask.md`; this file is about what they look like.

## Screenshots before and after

`just screenshots <base-url> <label>` (`scripts/screenshots.rs`) shoots every page of a running `retrodoc ui` with
headless Chrome, at 1440 px and 500 px wide (the narrowest window headless Chrome draws), into
`<runs directory>/screenshots/<label>/<kind>-<width>.png` (the runs directory is `$RETRODOC_RUNS_DIR`, else
`~/retrodoc-runs`, outside git), after removing the shots an earlier run left under that label. It finds one domain,
feature, use case, domain map, code page and collected doc from the links of the home page, the fixed pages and the
pages they lead to, and searches the first word of the first domain; it warns about a kind of page it found no link
to, and stops on a page that does not answer 200.

Shoot the same docs before and after the change, with the ask page served (`--ask`; no question is asked, so no LLM
call is made, a placeholder key is enough):

```sh
DEEPSEEK_API_KEY=unused cargo run -p retrodoc-cli -- ui --ask --path <docs repo> --bind 127.0.0.1:8730
just screenshots http://127.0.0.1:8730 before-<commit>
```

The stylesheet is embedded in the binary: rebuild before the "after" shots, and hard-reload a browser that keeps
an older one cached.

## Design variables

`:root` of `crates/retrodoc-web/assets/style.css` holds the colours of the charter (dark surfaces, gold as the one
accent, kept rare; `--gold-dark` never for text; `diagrams.js` copies the colours, keep them in sync), the
spacing steps `--space-1` to `--space-6`, the radius `--radius-sm`, `--radius`, `--radius-pill`, the type scale
`--text-xs` to `--text-2xl` and `--control-height`. A component uses them rather than a literal value.

## Components

| component | markup | look |
|---|---|---|
| button | `<button>` (or `.button` on a link) | primary: gold, the one action of a form |
| | `class="secondary"` | on the surface: filter, show |
| | `class="quiet"` | text only: revert, start over |
| section actions | `{% call m::section_actions(href) %}` under the heading of a section page, on the rows of the debt list | a primary "Review" (`#review`) and a secondary "Edit" (`?edit=1#edit-form`, the form unfolded); on a row of a list (`in_list`), a secondary "Review" and a quiet "Edit", so that the gold stays rare |
| field | `<label class="field"><span class="field-label">…</span> <control></label>`, `.hint` below | label above the control; `.field.wide` grows in a line of fields |
| form layouts | `.filters` | fields on one line, the button at their foot |
| | `form.compare` | the runs as `table.runs` with a `from` and a `to` radio per row, the compared rows lit, the swap link below |
| filter | `{% call m::filter_fields(filters) %}` in `form.filters`, `{% call m::active_filters(n, reset) %}` atop the list | the kinds as `fieldset.chips` of `.chip` radios, the others as selects; the checked chip and a select off "All" lit gold; the button in `<noscript>` |
| | `.edit-fields`, `form.ask` | fields in a column, `.actions` for the buttons |
| | `section.review`, `section.edit-form` | a panel on the surface |
| table | `<table>`, `<thead>` | muted header, row lit on hover (not the heatmap nor the code) |
| badge | `.badge` (`low`, `stale`, `review …`, `edited`) | pill, the label carries the meaning |
| card | `.card`, `.start-links li` | surface, border lit on hover |
| empty state | `<p class="empty">` | a dashed frame, muted text; says why it is empty and what to run or open |
| main action | `<p class="main-action">` under the heading of the home page | one primary button: the debt while it has items, else the map |
| form error | `{% call m::form_error(error) %}` above the buttons; `#ask-error` on the ask form | framed, the gold edge at its left; hidden when empty |
| page error | `#page-error` at the top of `<main>`, filled by `assets/forms.js` | same frame, for an error answer that is not HTML |
| notice | `{% call m::notice(saved, text) %}` above the buttons | gold-light word that fades out in 4 s (stays with reduced motion) |
| composer | `form.ask textarea`, `.keys` beside the button, `assets/ask.js` | one line growing to 12 (`field-sizing: content`, a script fallback); Enter sends, Shift+Enter breaks the line |
| suggestions | `ul.suggestions`, `button.secondary.suggestion` above the ask form | secondary buttons that wrap, normal weight; only on an empty conversation |
| indicator | `<span class="indicator" role="status">` in the form, `hx-indicator="find .indicator"` on it (`next .indicator` on the search box, which sends itself) | hidden; a spinner and a muted word while the request runs |
| hits panel | `#search-panel.search-panel` in `form.search`, filled with `search_panel.html` | under the box, on the surface; a row per hit: title (two lines at most), muted kind, badge; the gold link to the full page at its foot; hidden while empty; the box a combobox, `assets/search.js` for `/`, the arrows (the active hit lit, a gold edge at its left), Enter and Escape |

Tests find some forms and buttons by their exact markup (`<form class="filters"`, `<button type="submit" disabled>`):
add a variant class after the existing attributes.

## States of the forms

Each form shows each state that applies to it; a cross marks the states it must show. Their behaviour is WEB-102 to
WEB-106 and WAK-40; `assets/forms.js` swaps the error answers that name their swap (`HX-Reswap`), clears the errors of a form at each send and drops `?saved` from the address bar.

| form | page | rest | hover | focus | disabled | pending | saved | error |
|---|---|---|---|---|---|---|---|---|
| search (top bar) | every page | x | | x | | x | | |
| ask (question, send, start over) | ask | x | x | x | x (a question runs) | x | | x |
| review (status, comment, save) | domain, feature, use case | x | x | x | | x | x | x |
| edit (name, description, narrative, save, revert) | domain, feature, use case | x | x | x | | x | x | x |
| debt filters (kind chips, domain) | debt | x | x | x | | x | | |
| rules filters (kind chips, domain, feature) | rules | x | x | x | | x | | |
| run comparison (from, to, swap) | history | x | x | x | | x | | |

- **rest**: the control at rest, its label above it, its hint below.
- **hover**: a button, a chip or a select changes its border or background under the pointer.
- **focus**: a 2 px gold outline on the control the keyboard reached (`:focus-visible`).
- **disabled**: dimmed, the pointer not allowed, no hover.
- **pending**: the request is running: the submit button disabled, an indicator shown.
- **saved**: a short notice that the change was written, faded out after a few seconds.
- **error**: the message beside the field, the input kept.
