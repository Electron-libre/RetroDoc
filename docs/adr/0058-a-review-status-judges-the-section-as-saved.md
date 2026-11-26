# 0058. A review status judges the section as saved

Status: Proposed

## Context

A review's status (`validated`, `wrong`, `to_review`) can judge a section as the run generated it or as the form
saves it, with the human edit saved beside the review. The two readings disagreed: the settling agent proposed
`wrong` whenever it corrected a field, and `propose_verdict` refused `validated` with a correction (ADR 0057), while
the reader, in each of the three settling measures, saved `validated` after the correction (`docs/findings.md`,
section 4, item 5). They also differ for the next run: a review `wrong` makes the next `generate` ask its unit again
with the comment (ADR 0056), while an edit is kept over what the unit writes, so a section saved `wrong` with its
correction pays a call to rewrite what the reader already fixed.

## Decision

This amends ADR 0057. A status judges the section as saved, its edit applied: `validated` says the section is right
as saved, with or without the corrections saved with it; `wrong` says a claim of it is wrong and not corrected, so
the next `generate` writes it again; `to_review` says it is not settled. `propose_verdict` proposes `validated` with
its corrected fields and refuses `wrong` with a corrected field, its error naming `validated`. The form says under
its statuses what each one means for the section as saved.

Rejected: the status judging the section as generated (`wrong` with its correction), which matches neither the
reader's reading nor the cost of the next run, unless the run learns to skip a review `wrong` that an edit answers:
a second rule of when a review is answered, beside the one of ADR 0056, for the same outcome as `validated`.

## Consequences

The next run reads a corrected section as validated with its edit (ADR 0056: the edit and the section given as the
text to keep) and asks no unit again for it. A section the form saves `wrong` with an edit is still written again,
which the hint of the form tells the reader; the form does not refuse it. The reviews saved before this decision keep
their status: a `wrong` with an edit saved earlier asks its unit once more, as before.
The counts of the reviews by status (the audit page, the history) no longer tell how often the generated text was
wrong: a section corrected and saved `validated` counts as one validated untouched; its edit tells them apart.
