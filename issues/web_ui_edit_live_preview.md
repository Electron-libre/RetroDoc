---
priority: low
depends_on: [web_ui_form_feedback]
why: 'a comfort for editing, which works without it'
---
# Preview an edited narrative as it is typed

# Goal
The edit form of a domain, a feature or a use case shows plain text areas; the narrative is Markdown but the user
sees how it renders only after saving. Reverting to the generated text is one click with no confirmation. Expected: a
preview of the narrative rendered on the server while typing, side by side or in a tab, and a confirmation before
the revert.

# Findings
* `templates/edit_form.html`: inputs `name`, `description`, `narrative` (8 rows) and a "revert" button sent with
  `name="action" value="revert"`.
* `pulldown-cmark` is already a dependency of `retrodoc-web` (collected docs page, WEB-47 to WEB-50, with raw HTML
  shown as text).

# Approach
1. A `POST /preview` (or a fragment route) that renders the Markdown with the same rules as the collected docs
   (escaped raw HTML, unsafe links as text), triggered with `htmx` on `input changed delay:500ms`.
2. `hx-confirm` on the revert button, or a confirmation step in the fragment.

# Specs
* `specs/section-edit.md` or `specs/web-ui.md`: the preview route and its escaping, the confirmation of the revert.

# Milestone
A web interface that invites to act

# Resources
* `templates/edit_form.html`, `edit.html`; `crates/retrodoc-web/src/edit_form.rs`, `edit.rs`, `collected.rs`
* `specs/section-edit.md`, `specs/web-ui.md` (WEB-10 escaping, WEB-49, WEB-50); test `edit_pages.rs`

# Hints
* The preview route must not write anything (WEB-12).

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Only a use case has a narrative (SED-3), and its page shows it today as plain text (`<p class="narrative">`), not as
Markdown: a Markdown preview would differ from the page. Means: render the narrative of the page as Markdown by the
rules of the collected docs (WEB-118), preview it by the same function (`POST /preview`, WEB-119, WEB-120), confirm the
revert with `hx-confirm` (WEB-121). Out of scope: a preview of the name or the description (plain text).

## Plan
1. [x] Narrative as Markdown: the use case page renders its narrative with `collected::markdown_html`; verified by a
   test of `item_pages.rs` (escaped `<script>`, dropped `javascript:` link); makes true WEB-118. (autonomous)
2. [x] Preview: `POST /preview` answering the rendered fragment, the preview block in the edit form of a use case
   filled at render and wired with `hx-post`/`hx-trigger`; verified by tests of `edit_pages.rs` (fragment, escaping,
   empty narrative, no file written, no preview on a domain or feature form); makes true WEB-119, WEB-120. (autonomous)
3. [ ] Confirm the revert: `hx-confirm` on the revert button with a text of `texts.rs`; verified by a test of
   `edit_pages.rs`; makes true WEB-121, spec back to `accepted`.

## Decisions
- Mode: autonomous
- Base commit: 2c19f938a081e6617e5f16059c64e4aaad3e22f5
- 1 a: the use case page renders its narrative as Markdown, the preview by the same function.
- 2 a: `hx-confirm` on the revert button.

## Deliverable 1
done: the use case page renders its narrative from Markdown (`collected::markdown_html`), `<div class="narrative">`.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (WEB-118 to WEB-121 drafted) |
| docs | `docs/crates/render-mcp-cli.md`, `collected.rs` doc |
| code review | see below |

red: WEB-118: no `<div class="narrative">` in the page (the narrative was a `<p>` of escaped text).

Code review: no findings

not verified: the look of a narrative with lists or headings in a browser.

decisions needed: none

## Deliverable 2
done: `POST /preview` renders the narrative sent (`edit_form::preview`); the edit form of a use case holds the preview
under its narrative, asked again on `input changed delay:500ms`.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md`, `docs/dev/web-ui.md` |
| code review | see below |

red: WEB-119, WEB-120: no preview in the edit form; `POST /preview` answered the not-found page.

Checked in headless Chrome on a copy of a saved run (`~/retrodoc-runs/live-preview-check`): typing updates the preview
(bold, list, raw HTML as text), the buttons of the form stay enabled.

Code review: no findings

not verified: the preview on a 500 px wide window.

decisions needed: none
