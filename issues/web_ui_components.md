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
