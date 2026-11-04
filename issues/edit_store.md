---
priority: high
why: 'every other issue of the milestone reads or writes the edits it stores'
---
# Store the human edits of the docs sections

# Goal
A team can only take the generated docs as the model wrote them: nothing keeps a correction across runs. Expected:
an edit of a domain, a feature or a use case (its name, description and narrative), with its author and its date,
kept in a file of `.retrodoc/` outside the cache, read and written by the other crates, so that the web form, the
read model, the rendered Markdown and the MCP answers share one record and no `generate` loses it.

# Findings
* 2026-11-05: the reviews already live this way (`retrodoc_mcp::reviews`, ADR 0044): one versionable file,
  `.retrodoc/reviews.json`, written whole aside then renamed, the author from the git `user.name`, the fingerprint
  of the section and the hashes of the files it cites when given (REV-6, REV-19).
* 2026-11-05: decided with the user: only the name, the description and the narrative are edited (not the steps,
  the rules nor the glossary); the edits are an overlay kept apart, the generated text left as it is beneath.

# Approach
1. A model in `retrodoc-mcp` (an `edits` module next to `reviews`): per section (kind and id of RHI-7), the edited
   fields only (a field left out keeps the generated text), the author, the date, the fingerprint of the generated
   section and the hashes of its cited files when the edit was made (what `edit_outdated` compares).
2. One file, `.retrodoc/edits.json`, saved like the reviews (aside then renamed, an unreadable file reported and
   never overwritten, one save at a time in a process); share the save with `reviews` rather than copy it.
3. A new edit of a section replaces its previous one; removing it brings the generated text back.

# Specs
* `specs/section-edit.md` (new): the model, the file, the fields, the replacement.

# Milestone
Edit the docs

# Resources
* `crates/retrodoc-mcp/src/reviews/mod.rs` (the store to mirror), ADR 0044, `specs/section-review.md`.
* A new ADR: the edits as an overlay over the generated artifacts, outside the cache.
