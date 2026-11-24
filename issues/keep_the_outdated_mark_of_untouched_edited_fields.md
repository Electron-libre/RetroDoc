---
priority: low
why: 'a corner of the outdated edits: the reader loses a warning only when correcting another field of an outdated edit'
---
# Keep the outdated mark of the edited fields a save leaves untouched

# Goal
Saving the form of a section whose edit is outdated, with one field changed, records the whole edit again (SED-15),
its untouched fields included, with the fingerprint of the section as generated now: the stale fields are no longer
said outdated, and the reader is never told they may be wrong. Expected: a field the reader did not touch keeps
telling that the generated text changed under it.

# Findings
* Found by the `high` review of `issues/done/one_form_to_review_and_edit_a_section.md` (2026-11-24); the former edit form
  behaved the same.
* Example: a use case whose description is edited "OldD"; a later run rewrites its generated description, so the
  edit is outdated (SED-34). The reader corrects only the name and saves: the edit `{name, description: OldD}` is
  recorded with the new fingerprint and is no longer outdated.

# Approach
1. Keep the outdated mark per field: an edit holds, per field, the generated text or fingerprint it corrects.
2. Refuse or warn: a save that keeps the fields of an outdated edit says so in the form, the reader confirming them.
3. Show, in the form, the new generated text beside each stale field, as the page does (WEB-92).

# Specs
* `specs/section-edit.md` SED-15, SED-34; `specs/section-form.md` SFM-3.

# Milestone
Read by someone taking an application over

# Resources
* `crates/retrodoc-web/src/section_form.rs` (`edit_change`), `crates/retrodoc-docs/src/edits/mod.rs` (`record`,
  `outdated`), `crates/retrodoc-docs/src/corrections/mod.rs`.
