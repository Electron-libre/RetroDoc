---
priority: medium
depends_on: [edit_store, edits_in_read_model]
why: 'an edit made on code that changed since must not keep standing for it unseen'
---
# Flag an edit whose section or code changed since it was made

# Goal
An edit corrects a section as the run wrote it. A later `generate` can rewrite the generated section, and the code
it cites can change. Expected: the edit is kept but shown outdated, with the new generated text beside it, so the
team decides whether to update or drop its correction (decided with the user on 2026-11-05).

# Findings
* 2026-11-05: `retrodoc_mcp::reviews::outdated` tells a review outdated from the fingerprint of its section and the
  hashes of the files it cites (REV-19 to REV-22), reading only the files the section cites now.

# Approach
1. Outdated when the fingerprint of the generated section differs from the one the edit saved, or when a file it
   cites changed since the edit; share the rule with the reviews rather than copy it.
2. The page shows the edit outdated with the new generated text; the debt lists an outdated edit; the MCP answers
   say the text is a human edit, outdated when it is.

# Specs
* `specs/section-edit.md`: when an edit is outdated.
* `specs/debt-view.md`, `specs/web-ui.md`, `specs/mcp-server.md`: an outdated edit.

# Milestone
Edit the docs

# Resources
* `crates/retrodoc-mcp/src/reviews/mod.rs` (`outdated`, `cited_paths`), `issues/done/review_outdated.md`.

# Hints
* The fingerprint of an edit must be taken on the generated section, not on the edited one: comparing the edited
  text with itself never tells a rewrite.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
Tell an edit outdated (its generated section rewritten, or a file it cites changed) by the rule of the reviews, and
show it: an item of the debt, a line of the MCP answers with the generated text, a mark on the page with the
generated text. Out of scope: the edit form (`web_edit_form`), the Markdown mark (`edit_mark_in_markdown`).

## Plan
1. [x] Rule and debt: `reviews::outdated` split into a shared judgement over a fingerprint and file hashes, against
   the sections as shown (review) or as generated (edit); `edits::outdated`; the debt kind "outdated edit" (key,
   weight, order, web label); verified by tests per requirement; makes true SED-34 to SED-37, DBT-39. (autonomous)
2. [x] MCP: the edit line of `get_domain`, `get_feature`, `get_use_case`, outdated with its reasons and the generated
   text at the end; verified by tool tests; makes true MCP-15, MCP-16. (autonomous)
3. [ ] Web: the edit of a section on its page (fields, author, date), flagged outdated with the generated text beside
   each field; verified by page tests; makes true WEB-91, WEB-92.

## Decisions
- Mode: autonomous
- Base commit: e9f3a76405ea0b4287afbb7ea315522d4f18e159
- 1 a: an outdated edit weighs 0.5 in the debt; 2 a: this issue shows the edit on the page, read only; `web_edit_form` adds the form (its approach amended).

## Deliverable 1 summary
- done: `reviews::judge`, the rule of `reviews::outdated` over a fingerprint and file hashes against given sections;
  `edits::outdated` on it against the generated sections; `DebtKind::OutdatedEdit` (key `outdated_edit`, weight 0.5,
  after "to review", a heatmap column); `OutdatedView::of_edit` and `EditTexts` ("Outdated edit … since the edit");
  specs SED-34 to SED-37, DBT-39, DBT-29 and AUD columns amended; the MCP and web amendments (MCP-15, MCP-16, WEB-91,
  WEB-92) wait for deliverables 2 and 3.
- red: DBT-39: `assertion left == right failed: left 0, right 1` (no "Outdated edit" item); SED-34 to SED-37:
  written with `edits::outdated`, red as a compile error before it existed (not run red apart).
- Code review: no findings
- not verified: none.

## Deliverable 2 summary
- done: `tools/edit.rs`, the `Edit:` line of `get_domain`, `get_feature`, `get_use_case` after the review line (the
  fields, the author, the date, `outdated: …` with `Outdated::edit_reasons`, "since the edit"), and `## Generated
  text` ending the answer of an outdated edit; `OutdatedReview::reasons_since`; the MCP-14 stdio test reads its
  outdated edit's generated text at the end only; spec MCP-15, MCP-16.
- red: MCP-15, MCP-16: the answers had no `Edit:` line (`assertion failed: out.contains(...)`).
- Code review: no findings
- not verified: none.
