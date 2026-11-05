---
priority: low
depends_on: [web_edit_form]
why: 'the edited text already reaches the Markdown; the mark only tells a reader of the docs folder who wrote it'
---
# Mark the edited sections in the rendered Markdown

# Goal
`render` and `generate` write the human edits in place of the generated text (SED-30), with nothing telling a
reader of the docs folder that a person wrote it, nor who nor when. Expected: an edited section of the rendered
Markdown says it was edited by a human, with the author and the date, as the web page will (`web_edit_form`), and
the render stays deterministic.

# Findings
* 2026-11-05: decided with the user while planning `edits_in_read_model`: no mark in the Markdown in that issue; no
  issue of the milestone covers it (the page is `web_edit_form`'s, the MCP answers `edit_outdated`'s).
* 2026-11-05: `publish` (`crates/retrodoc-cli/src/commands/docs.rs`) lays the edits over the sections with
  `retrodoc_mcp::edits::apply` before `retrodoc_render::render_with_docs`, which never sees an edit.

# Approach
1. Pass the edits, or a per-section mark (author, date), to the render, and write one line under the heading of an
   edited domain, feature or use case, in the docs language (`Texts`).
2. Reuse the wording of the web page of `web_edit_form` so the two readers say the same thing.

# Specs
* `specs/section-edit.md`: the mark of an edited section in the rendered Markdown.

# Milestone
Edit the docs

# Resources
* `crates/retrodoc-cli/src/commands/docs.rs` (`publish`), `crates/retrodoc-render/src/markdown/`,
  `crates/retrodoc-render/src/texts.rs`, `issues/web_edit_form.md`.

# Hints
* The same artifacts and edits must render the same bytes (SED-31): the date is the edit's, not the render's.
