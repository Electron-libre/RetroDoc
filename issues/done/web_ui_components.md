---
priority: high
why: 'every other issue of the milestone builds on these components and on the screenshots taken before'
---
# Give the web interface one set of current components

# Goal
The web interface styles its controls in a few lines: buttons, fields, selects and text areas share one rule
(`style.css`, `input, select, textarea, button`), with no hover, disabled or primary/secondary distinction, no
spacing or type scale, and each page lays its forms out on its own. Expected: one set of components (buttons primary,
secondary and quiet; fields with their label and hint; selects; text areas; tables; badges; cards; empty states)
built on design variables (spacing, radius, type scale, the colours of the charter), used by every template, with
the screenshots of every page taken before the change to compare with after.

# Findings
* `crates/retrodoc-web/assets/style.css` is 202 lines; the form controls get colour, background, border, radius and a
  1 px gold focus outline, nothing else (no padding, no hover, no disabled state).
* The forms (ask, review, edit, debt and rules filters, run comparison, search) each use their own class
  (`.filters`, `.edit-fields`, `form.ask`, `.compare`) or none; the ask page's "start over" and the edit form's
  "revert" are buttons that look like the submit button.
* Cards already exist for the home page (`.card`, `.start-links`) but not elsewhere.
* No screenshot of the pages has been taken so far; Chrome headless is on the machine.

# Approach
1. Take the "before" screenshots: every page of `retrodoc ui` on `delivery_router` (home, domain, feature, use case,
   files, code, glossary, rules, actors, debt, audit, history, map, search, ask), at one desktop and one narrow
   width, with a small script (a `just` recipe or a script under `scripts/`) that can be rerun for "after". Write the
   checklist of the states each form must show (rest, hover, focus, disabled, pending, saved, error).
2. Design variables in `:root` (spacing steps, radius, type scale, the existing colours), then the components: buttons
   (`.button`, primary in gold, secondary, quiet for the destructive or secondary actions such as revert and start
   over), fields (label above, hint below), selects, text areas, tables (header, hover row), badges, cards, empty
   state block.
3. Apply them to every template (`macros.html` macros for the button and the field if it removes repetition), then
   the "after" screenshots.

# Specs
* `specs/web-ui.md`: probably none changed (the look is not a requirement); a requirement only if a template gains a
  behaviour.

# Milestone
A web interface that invites to act

# Resources
* `crates/retrodoc-web/assets/style.css`, `crates/retrodoc-web/templates/`, `templates/macros.html`
* ADR 0039 (server-rendered, vendored assets), `docs/crates/render-mcp-cli.md`
* `specs/web-ui.md` (WEB-69 credit line, WEB-11 assets served locally)

# Hints
* Keep the charter: dark surfaces, gold as the one accent, rare; `--gold-dark` never for text (comment in
  `style.css`); `diagrams.js` copies the colours, keep them in sync.
* The stylesheet is embedded in the binary: a browser may keep an older one cached, check with a hard reload.
* Over ~1,000 lines, split: the baseline screenshots and the variables in one deliverable, the components applied to
  the templates in another.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
One set of components (buttons primary/secondary/quiet, fields, selects, text areas, tables, badges, cards, empty
state) on design variables, used by every template, with before/after screenshots. Out of scope: the pending, saved
and error states of the forms (`web_ui_form_feedback`), only listed in the checklist here. No spec changes.

## Plan
1. [x] Baseline and variables: `scripts/screenshots.rs` (rust-script: from a running `retrodoc ui`, finds one page
   of each kind from the links of the home page, shoots each at 1440 px and 390 px with headless Chrome into a
   directory of the runs directory, outside git), its test of the page discovery, the recipe `just screenshots`, the
   checklist of the states of each form in `docs/dev/web-ui.md`, the design variables in `:root` (spacing,
   radius, type scale; no visible change); "before" screenshots of `delivery_router` taken; verified by
   `scripts/test_screenshots.rs` and the screenshots; ~400 lines. (autonomous)
2. [x] Components: buttons, fields, selects, text areas, tables, badges, cards, empty state in `style.css`, the
   macros `button`/`field` in `macros.html` where they remove repetition, applied to every template; "after"
   screenshots compared with "before"; verified by the web tests (`cargo test -p retrodoc-web`) and the screenshots
   at both widths; ~700 lines. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: bc18817a20fa2eb2d2937d4ec3532d9e5f8e1bbf
- Screenshots are taken on the English `delivery_router` run `docs_language-6248f5d-en` of the runs directory
  (no LLM call; the ask page is shot empty, without `--ask` questions asked).
- The script lives in `scripts/`; `just test-skill-scripts` also runs `scripts/test_*.rs`.

## Deliverable 1: baseline and variables
- done: `scripts/screenshots.rs` and its test, `just screenshots`, `scripts/test_*.rs` run by `just test-skill-scripts`,
  `docs/dev/web-ui.md` (screenshots, variables, states of the forms), the design variables in `:root`; "before"
  screenshots in `screenshots/before-bc18817` of the runs directory (17 pages at 1440 and 390 px; no collected doc
  linked in that run).
- checks: project-check green, `just test-skill-scripts` green, size 315 lines.
- red: none (no spec; `test_screenshots.rs` failed with "No such file" before `screenshots.rs` existed).
- Follow-up seen on the screenshots: at 390 px the tree beside the content leaves the main column ~25 px wide; the
  layout of a narrow width goes in deliverable 2 (a media query stacking the tree above the content).
- Code review: 1 finding. Fixed: a PNG left by an earlier run under the same label could pass for a new shot; the
  file is now removed before Chrome runs.
- not verified: none. decisions needed: none.

## Deliverable 2: components
- done: the components in `style.css` (headings, buttons primary/secondary/quiet with hover, focus and disabled,
  fields with label and hint, selects with their arrow, text areas, tables with a muted header and a lit row,
  badges as pills, cards, `.empty`, the form layouts, the review and edit forms in a panel, a narrow layout
  stacking the tree above the content); applied to every template with a form or an empty state; the macro
  `filter_fields` shared by the debt and rules filters; `docs/dev/web-ui.md` lists the components. "After"
  screenshots in `screenshots/after-d2` of the runs directory, compared with `before-bc18817`.
- checks: project-check green (the web tests unchanged and passing), size 233 lines (+ docs).
- red: none (a look, no requirement; the web tests guard the markup they assert, kept as is).
- not verified: the run comparison form (that run has no saved runs, so the history page does not show it); the
  hover and focus states seen only in the CSS, not on a screenshot.
- Code review: no findings.
- decisions needed: none.

## Deliverable 3: fixes of the `high` review
- done: the narrow shot is 500 px, the narrowest window headless Chrome draws (390 px rendered a 500 px page
  cropped); a label's earlier PNGs are removed before shooting; a page answering other than 200 stops the script;
  discovery also reads the fixed pages; the header links wrap on a narrow screen; the empty state on the map, the
  domain map and the no-run fragments of the rules, debt and search; the card and start-link literals moved to the
  variables; two redundant selectors dropped; the select arrow's copy of `--text-muted` noted beside it. "Before"
  retaken at 500 px from a build of `bc18817` (`screenshots/before-bc18817`), "after" in
  `screenshots/after-components` of the runs directory.
- Not fixed: one Chrome process per shot (36 cold starts, about two minutes a set): a tool run by hand before and
  after a change, the time is acceptable; `.button` kept with no user yet, for the links that act
  (`web_ui_next_action`).
- checks: project-check green, `just test-skill-scripts` green.
- red: none.
- Code review: no findings. (autonomous)
