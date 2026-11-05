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
  `crates/retrodoc-render/src/texts.rs`, `issues/done/web_edit_form.md`.

# Hints
* The same artifacts and edits must render the same bytes (SED-31): the date is the edit's, not the render's.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
`publish` passes a mark per edited section (fields, author, date) to the render, which writes one line under the
heading of the page of an edited domain, feature or use case, in the words of the web page (SED-38). Out of scope:
the mark in lists and `llms.txt`, the outdated state (web only).

## Plan
1. [x] Edit mark in the Markdown pages: `EditMarks` (by `ItemRef`) in `retrodoc-render`, a new argument of
   `render_with_docs`, the wording (`Human edit`, field names, `by`) moved from the web `EditTexts` into the render
   `Texts` and read by the web page through `t.doc`; `publish` builds the marks from the edits; verified by a render
   test and a CLI `render` test; makes true SED-38. ~340 lines. (autonomous)
2. [x] Docs of the whole issue and fixes of its `high` review; verified by the SED-38 tests. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: c86f28ecadb0089a8c3ea6ee0d2d1a80f1686102
- 2026-11-05 (user, `ok`): the mark on the page of the section only, not in the lists nor `llms.txt`; no outdated
  state in the Markdown (web only).

## Deliverable 1 summary
done: `**Human edit** <fields> *by <author>, <date>*` under the heading of the page of an edited domain, feature or
use case; the web page reads the same words from the render `Texts`; `section-edit` back to `accepted`.

| check | result |
|---|---|
| tests | pass (`project-check`) |
| lint | pass |
| specs | pass, `section-edit` accepted |
| docs | README (`render`), `docs/crates/render-mcp-cli.md` |
| code review | 1 finding, fixed |

red: SED-38: `left: "[← Domain overview](README.md)"`, the line under the heading of the feature page.
Code review: 1 findings: `edit_marks` inserted between the doc comment of `without_generated` and its `fn`; fixed,
moved above it.
not verified: none.
decisions needed: none.

## Deliverable 2 summary
done: `update-docs` over `c86f28e..HEAD` (nothing more: README and `render-mcp-cli.md` done in deliverable 1) and the
fixes of the `high` review.

| check | result |
|---|---|
| tests | pass (`project-check`) |
| lint | pass |
| specs | pass; the SED-38 scenario moved after SED-35 |
| docs | n/a, done in deliverable 1 |
| code review | high: 10 findings; low: no findings |

red: none (the hardened test of the author passes once `one_line` is applied; no new requirement).
High review, 10 findings:
1. author and date not on one line: fixed, `one_line` on both, the unit test gives an author with a newline.
2. an edited name with a newline cuts the heading: not fixed, the heading predates this issue (SED-30) and a
   generated name has the same risk; outside SED-38.
3. no outdated state in the mark: not fixed, decided with the user (web only).
4. `apply` and `changed` decide the fields apart: not fixed, both keep the non-blank fields and a narrative of a use
   case only; one would need `apply` to return marks, a change of the mcp API outside this issue.
5. `Field` → `EditedField` match in two crates: not fixed, neither crate may depend on the other's enum; five lines.
6. `mark_of` checks the fields again: kept as the guard of a public type, the doc now says a mark with none is not
   written.
7. `edited_fields` by position: not fixed, the pattern of the web `labels`; one table per language.
8. the CLI test rendered three times: fixed, one render.
9. an `ItemRef` built per page: not fixed, a few strings per page.
10. the SED-38 scenario out of order: fixed.
Code review: no findings
not verified: none.
decisions needed: none.
