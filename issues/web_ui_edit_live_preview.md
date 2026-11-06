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
