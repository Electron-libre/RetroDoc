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
