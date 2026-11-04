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
2. [x] Edits store: module `edits` (model, `record`, `remove`, `load`, `edits_path`) on `store`, spec
   `specs/section-edit.md` accepted, ADR 0045; verified by tests per requirement; makes true SED-1 to SED-22. (autonomous)
3. [x] Fixes of the `high` review of the issue; verified by the tests of SED and REV; makes true SED-18 (layout). (autonomous)

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

## Deliverable 2 summary
- done: module `retrodoc_mcp::edits` (`Fields`, `Edit`, `Edits`, `record`, `remove`, `load`, `edits_path`,
  `EditsError`) on `store`, `reviews::cited_hashes` shared with it; spec `section-edit.md` and ADR 0045 accepted;
  the `generate` test of the reviews also keeps an `edits.json` (SED-11); docs (ARCHITECTURE §2 and §7,
  `docs/crates/render-mcp-cli.md`).
- red: SED-1 to SED-10, SED-12 to SED-22: `not yet implemented` (stubs); SED-11 green at once (the file is outside
  the cache, as REV-8).
- Code review: no findings
- not verified: none.

## Deliverable 3 summary
- done: the fixes of the `high` review of the issue: the files saved in the order of the structs (`Saved`, version
  first; `serde_json::Map` sorted the keys), the path in the save error (`StoreError::Io`), `UnknownSection` out of
  the store (`ReviewsError` and `EditsError` each hold it), the author and the hashes read before the lock, a missing
  records key named in the load error, no lifetime on `Store`.
- red: SED-18: the layout assertion added to `the_edits_are_saved_by_kind_then_id` (keys in struct order), which the
  `Map` of the save sorted alphabetically.
- Code review: 9 findings (high review of the issue): fixed 1 (key order), 3 (path in the save error), 4
  (`UnknownSection`), 5 (I/O out of the lock), 6 (missing key), 7 (lifetime); not fixed 2 (fingerprint of edited
  docs): no reader applies the edits yet, logged in `issues/edits_in_read_model.md`; 8 (`edits()`/`reviews()`
  forward to `all()`): `reviews()` has callers in three crates, `edits()` mirrors it; 9 (trim): the form decides,
  logged in `issues/web_edit_form.md`.
- not verified: none.
