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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
A store of the human edits (name, description, narrative of a use case) per section, in `.retrodoc/edits.json`,
loaded and saved like the reviews. Out of scope: applying the edits (`edits_in_read_model`), the form, the outdated
check. Means as the issue's, the save shared with `reviews` through a new module.

## Plan
1. [x] Shared store: move the versioned load (version check, newest per section) and the save aside then rename of
   `reviews` into a `store` module generic over the record, `reviews` on it; verified by the REV tests unchanged and
   green; makes true no new requirement (refactor). (autonomous)
2. [ ] Edits store: module `edits` (model, `record`, `remove`, `load`, `edits_path`) on `store`, spec
   `specs/section-edit.md` accepted, ADR 0045; verified by tests per requirement; makes true SED-1 to SED-22.

## Decisions
- Mode: autonomous
- Base commit: 5ccc0602ff15409cc2283c59f345a8c912434a20
- 1 a: a blank field is refused (SED-5); 2 a: an edit that changes no field is refused (SED-4).

## Deliverable 1 summary
- done: module `retrodoc_mcp::store` (`Section`, `Record`, `Records`, `Store::load` and `Store::change`, `StoreError`),
  `reviews` on it (`ReviewsError` an alias of `StoreError`, `Reviews` of `Records<Review>`); the spec and ADR 0045
  drafts committed with it.
- red: none (refactor, the REV tests unchanged and green).
- Code review: no findings
- not verified: none.
