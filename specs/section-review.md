---
id: REV
status: accepted
adrs: [0039, 0043, 0044]
---

# Section review

## Intent

A human records what they checked in the generated docs: a domain, a feature or a use case marked validated, wrong
or to review, with a comment. The reviews are one record kept in the repository, which the web form, the debt, the
dashboard and the MCP answers read and write, so that a reader knows what a human has vouched for beside the
model's confidence.

## Requirements

- **REV-1** (deterministic) A review must name its section by its kind (domain, feature or use case) and its id, the
  id of the run history (RHI-7: the slug, `<feature>/<use case>` for a use case).
- **REV-2** (deterministic) A review must hold a status among `validated`, `wrong` and `to_review`.
- **REV-3** (deterministic) A review must hold its comment, empty when none was given.
- **REV-4** (deterministic) A review must hold as author the `user.name` of the git config of the repository, none
  when it is unset or the folder is not a git repository.
- **REV-5** (deterministic) A review must hold the date it was given in RFC 3339, in UTC, to the second.
- **REV-6** (deterministic) A review must hold a fingerprint of its section as the docs held it when the review was
  given, equal for two sections with the same content and different when any field of the section differs.
- **REV-7** (deterministic) The reviews must be saved in one file, `.retrodoc/reviews.json`, outside
  `.retrodoc/cache/`.
- **REV-8** (deterministic) A `generate`, `--force` included, must not delete nor rewrite `.retrodoc/reviews.json`.
- **REV-9** (deterministic) A save must write the whole file aside then rename it over the former one, so that no
  reader ever sees a file cut short.
- **REV-10** (deterministic) A file that cannot be read, or of a later format, must be reported as an error when the
  reviews are loaded.
- **REV-11** (deterministic) A save must not overwrite a file that cannot be read or is of a later format: it fails
  and leaves the file as it was.
- **REV-12** (deterministic) A review of a section must replace the previous review of the same section.
- **REV-13** (deterministic) Removing the review of a section must leave the section with no review, the other
  reviews kept.
- **REV-14** (deterministic) An absent file must load as no review.
- **REV-15** (deterministic) The reviews must be saved in a stable order (by kind, then id), so that a file under
  version control changes only on the lines of the reviews that changed.
- **REV-16** (deterministic) A review of a section the docs do not hold must be refused, and nothing saved.

## Scenarios

- (REV-12) Given a feature validated by A, when B marks it wrong, then the file holds one review of it, by B, wrong.
- (REV-13) Given a domain and a use case reviewed, when the review of the domain is removed, then only the review of
  the use case remains.
- (REV-11) Given a `reviews.json` holding invalid JSON, when a review is saved, then the save fails and the file is
  byte for byte the same.
- (REV-10) Given a `reviews.json` of format version 2 read by a version 1 reader, then loading fails, naming the
  version.
- (REV-8) Given a reviewed section, when `generate --force` runs with no LLM call needed, then the review is still
  in the file.
- (REV-6) Given a use case whose narrative was rewritten by a later run, then its fingerprint differs from the one
  saved with its review.
- (REV-1) Given two features of different domains sharing no slug, then a use case `a/b` and a feature `a` are two
  sections, never confused.

## Non-goals

- No review of a business rule nor of a glossary term (decided with the user on 2026-11-04).
- No history of the reviews: a new review replaces the former one; git keeps the history of a versioned file.
- No lock across processes: two writers at the same instant keep the last one's file whole (REV-9), not both.
- Telling a review outdated (`issues/review_outdated.md`), showing the reviews (`issues/web_review_form.md`,
  `issues/review_in_debt.md`, `issues/review_in_mcp.md`).
