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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-25

## Understanding
A save that keeps a field of an outdated edit as it was keeps, for that field, the fingerprint and file hashes the
former edit held for it, so it stays outdated; the form offers a checkbox to confirm an outdated edit, which re-bases
every field. Out of scope: marking which field is stale on the page (WEB-92 already shows the generated text beside
each field), merging with the new generated text.

## Plan
1. [x] Per-field base of an edit (`retrodoc-docs`): `Edit` gains an optional map, per field kept from an outdated
   former edit, of its fingerprint and file hashes (absent from the file otherwise, format version unchanged);
   `edits::record` keeps them for a field whose text equals the former edit's unless saved confirmed;
   `edits::outdated` judges the edit and each field base, reasons merged; `corrections::Correction` carries the
   confirmation. Verified by internal tests in `edits/tests.rs` and `corrections/tests.rs` and a test of the form
   through the router (name corrected, outdated description kept: still outdated); makes true SED-15, SED-34, SED-35,
   SED-36 (amended), SFM-3 (amended). ~350 lines. (autonomous)
2. [x] Confirmation in the form (`retrodoc-web`): an unchecked checkbox `confirm` in the form of a section whose
   edit is outdated, its label in English and French (a field of `EditTexts` in `texts.rs`, +4 lines; the split of
   that file left out, a precedent `settle_texts.rs` exists if it grows), a save with it checked writing the edit as
   sent confirmed, an unchanged one included. Verified by tests in a new `crates/retrodoc-web/tests/edit_confirm.rs`
   (`section_form.rs` is at 709 lines); makes true SFM-17; sets both specs `accepted`. ~250 lines. (autonomous)
3. [x] Docs of the whole issue and fixes of its `high` review. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 80809f28ccec21efb9a4de0660124ea3e305969d
- The rule is amended into SED-15 and SED-36 rather than new requirements: `specs/section-edit.md` is at the bound of
  40 requirements (`spec_max_requirements`).
- A field "kept" is one whose text, cleaned, equals the former edit's; only a base that is outdated at save time is
  stored, so an edit that was not outdated stores none (SED-36 unchanged for it).
- `edits.json` keeps `VERSION` 1 with `field_bases` (`high` review): a bump would make every older binary refuse the
  whole file (SED-13), a heavier loss than the per-field mark an older binary drops when it saves.
- Questions of step 2: 1 a (a checkbox `confirm` in the form, shown only when the edit is outdated), 2 a (SED-15 and
  SED-36 amended, no new id).

## Deliverable 1 summary
done: `Edit::field_bases` keeps the base of a field kept from an outdated former edit; `edits::record_confirmed` and
`EditChange::Confirm` re-base every field; `edits::outdated` merges the reasons of the edit and its field bases.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (SED, SFM `draft`) |
| docs | `docs/crates/docs.md` |
| code review | low, no findings |

red: SED-15, SED-34, SED-35: `left: None, right: Some(Outdated { rewritten: true, .. })` (the kept field lost its base)

Code review: no findings

not verified: none

decisions needed: none

## Deliverable 2 summary
done: the form of a section whose edit is outdated holds an unchecked `confirm` checkbox (English and French label);
a save with it checked writes the edit confirmed (`EditChange::Confirm`), an unchanged one included; both specs
`accepted`.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (SED, SFM `accepted`) |
| docs | `docs/crates/render-mcp-cli.md`, `docs/dev/web-ui.md`, `README.md` |
| code review | low, no findings |

red: SFM-17, SED-36: the page held no `name="confirm"` checkbox; `left: Some(Outdated { rewritten: true, .. }), right: None` after a confirmed save

Code review: no findings

not verified: the look of the checkbox (no screenshots shot); the checkbox kept checked in the form answered to a
refused save (SFM-15) has no test

decisions needed: none

## Deliverable 3 summary (docs and `high` review fixes)
done: a field the page left untouched is taken as the docs show it now (a concurrent edit kept, its base with it);
the kept fields compared cleaned; the former edit judged before the lock; tests tightened.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | high: 7 findings; low on the fixes |

red: SFM-3, SFM-17, SED-15: a confirmation from a stale page wrote its old name over the edit saved since (`high` review findings 1, 2)

Code review: 7 findings (high): 1, 2 fixed (untouched fields taken as shown now); 3 not fixed (see Decisions:
`VERSION`); 4 fixed (judged before the lock, again under it only when the edit changed since); 5 fixed (compared
cleaned); 6, 7 fixed (tests).

not verified: the look of the checkbox (no screenshots)

decisions needed: none
