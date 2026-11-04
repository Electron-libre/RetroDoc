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
