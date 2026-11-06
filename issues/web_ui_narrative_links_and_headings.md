---
priority: low
why: 'a narrative rarely holds a link or a heading; the page stays readable and safe'
---
# Resolve the links and the headings of a narrative rendered as Markdown

# Goal
Since WEB-118 the narrative of a use case is rendered from Markdown on its page, in the generated text of its edit
and in the preview of the edit form, by the function of the collected docs. Two rules of that function fit a
collected doc, not a narrative shown inside another page. Expected: a relative link of a narrative leads where the
Markdown docs would lead, and a heading of a narrative neither breaks the outline of the page nor duplicates an id.

# Findings
* `collected::markdown_html` keeps a relative link as written: on `/domains/<d>/<f>/<u>`, `[guide](docs/guide.md)`
  resolves to `/domains/<d>/<f>/docs/guide.md`, a 404, not to the collected doc at `/docs/docs/guide.md`. The ask page
  renders its answers with the same function and has the same trait.
* It gives each heading an `id` (`anchor`) and keeps its level: `# Title` becomes an `<h1>` inside "What happens".
  The page renders the narrative twice (body and preview of the edit form), so a heading's `id` appears twice.
* Found by the `high` review of `issues/web_ui_edit_live_preview.md`; no narrative of a saved run was checked for
  links or headings.

# Approach
1. Measure: count the narratives of the saved runs that hold a link or a heading.
2. Render a narrative with its own options: no heading ids, the headings shifted under the `h2` of the page (or
   shown as strong text).
3. Resolve a relative link against the path of the Markdown page of the use case and map it to `/docs/<path>` when a
   collected doc is there, else show it as text.

# Specs
* `specs/web-ui.md`: amend WEB-118 (headings, relative links).

# Milestone
A web interface that invites to act

# Resources
* `crates/retrodoc-web/src/collected.rs` (`render`, `narrative_html`), `view.rs`, `edit.rs`, `edit_form.rs`, `ask.rs`
* `specs/web-ui.md` WEB-49, WEB-50, WEB-118 to WEB-120
