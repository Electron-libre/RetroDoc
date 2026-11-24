---
id: SFM
status: draft
adrs: [0039, 0044, 0045]
---

# Section form

## Intent

The page of a domain, a feature or a use case in `retrodoc ui` holds one form to judge the section and correct its
text, so that whoever reads the docs validates a section, marks it wrong and fixes it, or leaves it to review, in one
save. It writes the review (`specs/section-review.md`) and the human edit (`specs/section-edit.md`) of the section.

## Requirements

### Behaviour

- **SFM-1** (deterministic) The page of a domain, a feature or a use case must hold one form, `#review`, sent with
  `htmx`, that holds the status of the review of its section (validated, wrong, to review, or none) as a group of
  radio choices, its comment, and its name, its description and, for a use case, its narrative, each filled with the
  text the page shows (SED-23), and one button that saves them.
- **SFM-2** (deterministic) The fields of the text (name, description, narrative) must be folded under the status
  and the comment, and unfolded when the page shows the section reviewed `wrong`, when the status `wrong` is chosen in
  the form, or when the form answers a save refused or failed (SFM-14).
- **SFM-3** (deterministic) A save must write the edit of the section, then its review: the edit only when a field
  sent differs, cleaned (SFM-10), from the text the page shows, an edit left as it was otherwise, an outdated one
  included (SED-34); the review as sent, a status recording it with the comment (REV-12), none removing it (REV-13).
- **SFM-4** (deterministic) A review saved with an edit must not be outdated by that edit: it holds the fingerprint
  of the section with the edit just saved applied (SED-26).
- **SFM-5** (deterministic) The form of an edited section must hold a button that brings the generated text back:
  it asks for a confirmation (its `data-confirm`, read by `forms.js`) in a text that says the edit is removed, then
  removes the edit of the section (SED-16) and writes the review as sent (SFM-3).
- **SFM-6** (deterministic) The page of a domain, a feature or a use case must show, right under its heading, one
  primary button "Review" that links to its form (`#review`).
- **SFM-7** (deterministic) Each item of the debt page whose section is a domain, a feature or a use case of the run
  must offer one action "Review", a link to the form of that section as SFM-6 gives it, labelled with the name of the
  section; an item of a rule, a folder or a section the run does not have offers none.
- **SFM-8** (deterministic) The page of a section asked with `?saved=1` must show in its form a notice that says the
  section is saved (`role="status"`); another value of `saved`, or none, shows no notice, nor does a form whose
  reviews cannot be read (WEB-87).

### Contract

- **SFM-9** (deterministic) The form must send its fields by a `POST` to the URL of the page, and a save be
  answered, with the header `HX-Request`, with the header `HX-Redirect` naming the page followed by `?saved=1`, so
  that the whole page shows what was saved, and without it, with a redirect `303` to the same URL.
- **SFM-10** (deterministic) A field of the text the form sends must be cleaned: its surrounding whitespace trimmed
  and the carriage return of each line end a browser sends (CR LF) dropped; the comment trimmed.
- **SFM-11** (deterministic) An edit written by a save (SFM-3) must leave out (SED-2) a field sent empty or equal,
  cleaned, to the text the last run generated for it; a save that leaves every field out removes the edit of the
  section (SED-16).
- **SFM-12** (deterministic) A save to a section the last run does not have, or before a first run, must be answered
  with the status 404, and nothing saved.
- **SFM-13** (deterministic) A save whose status is not one of the four of SFM-1, or that sends a narrative for a
  domain or a feature (SED-3), must be answered with the status 400, and nothing saved, neither the edit nor the
  review.
- **SFM-14** (deterministic) A save must be answered with the status 500 and the error when `.retrodoc/reviews.json`
  or `.retrodoc/edits.json` cannot be read or is of a later format, both files left as they were (REV-11, SED-14).
- **SFM-15** (deterministic) A save with the header `HX-Request` that is refused (SFM-12, SFM-13) or fails (SFM-14)
  must be answered, with its status, as the form of the section alone, its status and fields as sent, the error inside
  the form above its button (`role="alert"`), the header `HX-Reswap`, and above the form the review of the section as
  it is saved when the run has the section, swapped in place of the former form.
- **SFM-16** (deterministic) The form must hold an indicator that says, in the language of the interface, that it is
  being sent, shown while its request runs (`hx-indicator`), and disable its buttons while it runs
  (`hx-disabled-elt`).

## Scenarios

- **SFM-H1** (happy; SFM-1, SFM-3, SFM-6, SFM-8) Given a feature with no review, when the reader follows "Review",
  chooses "validated" and saves, then the feature is reviewed validated, its edit unchanged, and the page says it is
  saved.
- **SFM-H2** (happy; SFM-2, SFM-3, SFM-4) Given a use case with no review, when the reader chooses "wrong", writes
  the comment "the customer pays, not the clerk", corrects its description in the fields the choice unfolded and
  saves once, then the use case holds the edit and the review `wrong`, and the review is not outdated.
- **SFM-H3** (happy; SFM-7, SFM-3) Given a debt item of low confidence, when the reader follows its "Review", chooses
  "to review" with a comment and saves, then the section is left to review and its text unchanged.
- (SFM-3, SFM-4) Given a feature validated by A, when B corrects its name, the status `validated` kept as the form
  shows it, and saves, then the review is written again, validated by B, and not outdated.
- (SFM-3) Given a section whose edit is outdated, when the reader validates it without touching its fields, then the
  edit is left as it was, still outdated, and the review is not outdated.
- (SFM-3) Given a reviewed use case, when the form sends no status and its fields unchanged, then it has no review and
  its edit is as it was.
- (SFM-11) Given a feature whose name is edited, when the form sends the name emptied, then the feature has no edit.
- (SFM-11) Given a use case shown as generated, when the form sends its name and description unchanged and a new
  narrative, then the edit holds the narrative only.
- (SFM-10) Given a narrative of two lines typed in a textarea, ending with a line end, then the edit holds the two
  lines with `\n` between them and no line end after.
- (SFM-5) Given an edited feature reviewed wrong, when the reader brings the generated text back, then it has no edit
  and is still reviewed wrong.
- (SFM-13) Given a domain, when the form sends a narrative and the status validated, then the answer is 400 and
  neither `reviews.json` nor `edits.json` is written.
- (SFM-14) Given an `edits.json` holding invalid JSON, when a review only is saved, then the answer is 500 and both
  files are byte for byte the same.
- (SFM-15) Given a form whose status and name were changed, when the save fails, then the form answered holds the
  status and the name as sent and the error.
- (SFM-8) Given a section with no review, when its page is asked with `?saved=1`, then the form shows the notice and
  no review.
- (SFM-4) Given an outdated review, when the form saves the same status again, then the review is no longer outdated.

## Non-goals

- No edit of the steps, the business rules nor the glossary (`specs/section-edit.md`); no button per field brings
  one field back: emptying it does.
- No lock across the two files: a save that fails writing the review after its edit keeps the edit (the two stores
  each write whole files, SED-12).
- No form under `/edit`: the former path of the edit form serves no page; the preview of a narrative keeps its own
  path (WEB-119, WEB-120).
- No authentication of the reviewer: the author of the review and of the edit is the `user.name` of the repository
  (REV-4, SED-6).
- No word of an outdated review or edit in the form: the page shows them above it (WEB-89, WEB-92).
