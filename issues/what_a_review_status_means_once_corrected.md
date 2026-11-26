---
priority: medium
why: 'the settling and the reader disagree on the status of a corrected section, and `wrong` asks the next run to write it again'
---
# Settle what the status of a reviewed section means once it is corrected

# Goal
A review's status (`validated`, `wrong`, `to_review`, REV-2) can judge the section as generated or as saved with its
correction. The settling agent proposes `wrong` whenever it corrects a field, and `propose_verdict` refuses
`validated` with a correction (STL-13); the reader, in both measures, saved `validated` after the correction. The two
readings also differ for the next run: a section reviewed `wrong` is written again with its comment (HCR-4), while an
edit is kept. The project decides which reading holds, and the form, the settling and the next run follow it.

# Findings
* 2026-11-25, first settling measure: the one proposal saved, a domain, was saved `validated` with its corrected
  description although its comment said the reviewer's mark was right.
* 2026-11-25, second measure (`docs/findings.md`, section 4, item 5): the use case and the feature were proposed
  `wrong` with their corrected text; the reader saved both `validated` with the correction. The domain, with no
  proposal, was also saved `validated` with a description the reader corrected.
* A review `wrong` makes the next `generate` ask its unit again (HCR-4); a saved edit is kept over what the unit
  writes. Saving `wrong` with an edit would pay a call to rewrite a section the reader already corrected.

# Approach
1. Decide with the user which reading holds (a decision of the product, written as an ADR):
   1. The status judges the section as saved: a corrected section is `validated`; `wrong` means wrong and not
      corrected, to be written again. `propose_verdict` proposes `validated` with its correction.
   2. The status judges the section as generated: `wrong` with its correction; the next run must then not write
      again a `wrong` section whose edit answers it.
2. Align what the decision touches: STL-13 and the settling prompt, the form's wording of the statuses, HCR-4 if
   option 2.

# Specs
* `specs/section-settle.md` (STL-13), `specs/section-review.md` (REV-2, the meaning of the statuses),
  `specs/human-corrections.md` (HCR-4) if option 2.

# Milestone
Read by someone taking an application over

# Resources
* `specs/section-review.md`, `specs/human-corrections.md`, `specs/section-settle.md`, ADR 0057.
* `docs/findings.md`, section 4, item 5.

# Hints
* The reader of the measure is not a developer of the measured application: judge the method with them.

# Tracking

State: done: every deliverable committed.

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-26

## Understanding
The status judges the section as saved, its edit applied (option 1, ADR 0058): a corrected section is `validated`,
`wrong` means wrong and not corrected, written again by the next run (HCR-4 unchanged). `propose_verdict` proposes
`validated` with its correction and refuses `wrong` with one; the form says what each status means. Out of scope:
the logged reasons of the refusals (`issues/settling_proposal_refusals.md`), the reviews saved before.

## Plan
1. [x] Move the review texts out of `texts.rs`: `ReviewTexts`, `OutdatedTexts` and their English and French values to
   `crates/retrodoc-web/src/texts/review.rs`, no change of behaviour (`texts.rs` is over the file size limit);
   verified by the existing tests; ~250 lines. (autonomous)
2. [x] The settling follows the status as saved: `propose_verdict` accepts `validated` with corrected fields and
   refuses `wrong` with one, its error naming `validated`; its descriptions and the settling prompt say so; the ADR
   0058 and the spec drafts go with it; verified by the agent tests of the STL-13 scenarios and STL-H1; makes true
   STL-13, STL-5, STL-H1; ~150 lines. (autonomous)
3. [x] The form says what each status means: a hint under the statuses, English and French; verified by a web test
   of the form of a use case in both languages; makes true SFM-18; ~60 lines. (autonomous)
4. [x] Docs of the whole issue and the fixes of its `high` review: the fields of the text unfold on `validated` too
   (SFM-2, SFM-H2 saves `validated` with its edit), the tool fields never with `wrong`, the French hint names the
   section, no ADR number in the model's error, the prompt test checks the new meaning, the counts of the audit in
   the ADR's consequences, the finding of section 4 item 5 settled. (autonomous)
5. [x] A partial correction: `propose_verdict` accepts `wrong` with corrected fields, a claim left uncorrected; the
   prompt, the tool, STL-13 and ADR 0058 say so; verified by the agent test of STL-13; makes true STL-13; ~50 lines. (validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-26: answer b)

## Decisions
- Mode: autonomous
- Base commit: 998c83f7364bcc5c7934677fd9a679149c34129e
- 1 a: the status judges the section as saved (ADR 0058).
- 2 a: `propose_verdict` refuses `wrong` with a corrected field, its error naming `validated`. Reversed after the
  `high` review (finding 2, answered b on 2026-11-26): `wrong` is accepted with the fields it can correct, a claim
  left uncorrected; no status refuses corrected fields.
- The form refuses no save `wrong` with an edit: its hint warns; the reviews saved before keep their status.

## Summaries

### 1. Move the review texts
- done: `ReviewTexts`, `OutdatedTexts` and their English and French values moved to `texts/review.rs`, re-exported by
  `texts`; the spec drafts and the ADR 0058 draft go with it (the first deliverable).
- checks: tests, lint, specs pass (`project-check`); docs: no doc names the moved items by file.
- red: none (a move, no test).
- Code review: no findings
- not verified: none. decisions needed: none.

### 2. The settling follows the status as saved
- done: `propose_verdict` refuses `wrong` with a corrected field, its error naming `validated`, and accepts
  `validated` with one; its descriptions and the settling prompt say the status judges the section as saved; the
  fixtures of the agent and of the settle page propose `validated` with their correction, and STL-H1 checks the saved
  review is `validated`; `docs/crates/agent.md` names ADR 0058; STL back to `accepted`.
- checks: tests, lint, specs (`project-check`); docs: `docs/crates/agent.md`.
- red: STL-13: `called Result::unwrap_err() on an Ok value`: `wrong` with a corrected description was accepted.
- Code review: no findings
- not verified: what the model proposes under the new prompt (a paid measure, not run). decisions needed: none.

### 3. The form says what each status means
- done: a hint `p.hint.status-hint` under the statuses of the form (`ReviewTexts::status_hint`, English and French):
  validated is right as saved with the corrections, wrong is written again by the next `generate`, to review is not
  settled; `docs/dev/web-ui.md` and `docs/crates/render-mcp-cli.md` name it; SFM back to `accepted`.
- checks: tests, lint, specs (`project-check`); docs: `docs/dev/web-ui.md`, `docs/crates/render-mcp-cli.md`.
- red: SFM-18: `no hint under the statuses`.
- Code review: no findings
- not verified: the look of the hint in a browser (no screenshots taken). decisions needed: none.

### 4. Docs of the issue and fixes of the high review
- done: `forms.js` unfolds the fields of the text on `wrong` or `validated` (SFM-2, SFM-H2 now saves `validated`
  with its edit); the fields of `propose_verdict` say never with `wrong`; the French hint says the section is
  rewritten; the model's error cites no ADR; the prompt test checks "once corrected" and "cannot give its
  correction"; ADR 0058 notes the counts by status; `docs/findings.md` section 4 item 5 records the decision;
  `issues/settling_proposal_refusals.md` names the new refusal.
- high review: 7 findings; fixed 1 (form unfolds on `wrong` only), 3 (tool fields), 4 (French hint), 5 (ADR in the
  error), 6 (prompt test), 7 (audit counts, as a consequence of the ADR); 2 (a partial correction with `wrong`
  refused) left to the user: it reverses the decision 2 a.
- checks: tests, lint, specs (`project-check`).
- red: SFM-2: `UNFOLDING_STATUSES = ["wrong", "validated"] in <forms.js>` not found.
- Code review: no findings
- not verified: the unfolding in a browser (the test reads the script). decisions needed: finding 2 of the high review.

### 5. A partial correction
- done: `propose_verdict` accepts `wrong` with the fields it can correct, a claim left uncorrected (no status
  refuses corrected fields now); the prompt and the tool say so; STL-13, its scenario, ADR 0058,
  `docs/crates/agent.md`, `docs/findings.md` and `issues/settling_proposal_refusals.md` follow.
- checks: tests, lint, specs (`project-check`).
- red: STL-13: `called Result::unwrap() on an Err value: "Error: `wrong` means a claim of the section is wrong and left
  uncorrected..."`.
- Code review: no findings
- not verified: what the model proposes (a paid measure). decisions needed: none.
