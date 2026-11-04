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
- **REV-17** (deterministic) A file holding several reviews of one section (two teammates' files merged) must load
  as the newest of them only.
- **REV-18** (deterministic) Reviews saved at the same time by one process (two requests of `retrodoc ui`) must all be
  kept.
- **REV-19** (deterministic) A review must hold the hash of the content of each file its section cites, as the file
  was when the review was given: the source files of a feature, the files the steps of a use case cite, the source
  files of the features of a domain; a file that cannot be read gets no hash.
- **REV-20** (deterministic) A review must be outdated, with the reason "rewritten", when the fingerprint of its
  section in the docs now (REV-6) differs from the one it holds.
- **REV-21** (deterministic) A review must be outdated, with the files that changed, when a file it holds a hash of
  and its section cites now has another content or is deleted; a file that exists but cannot be read, or that the
  section no longer cites, is not judged.
- **REV-22** (deterministic) Whether a review is outdated must be judged when the review is read, never saved: an
  outdated review is kept as it was, and a new review of its section is up to date.

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
- (REV-4) Given RetroDoc run on a subfolder of a git repository, then the author is the `user.name` of that
  repository.
- (REV-3) Given a review written by hand with no `comment`, then it loads with an empty comment.
- (REV-1) Given two features of different domains sharing no slug, then a use case `a/b` and a feature `a` are two
  sections, never confused.
- (REV-21) Given a feature validated, when one of its source files is edited and no `generate` runs, then its review
  is outdated, naming that file.
- (REV-21) Given a review written by hand with no file hashes, then it is outdated only when its section was
  rewritten (REV-20).
- (REV-20, REV-21) Given a use case rewritten and a file it cites deleted, then its review is outdated for both
  reasons.

## Non-goals

- No review of a business rule nor of a glossary term (decided with the user on 2026-11-04).
- No history of the reviews: a new review replaces the former one; git keeps the history of a versioned file.
- No lock across processes: two processes saving at the same instant keep the last one's file whole (REV-9), not
  both.
- Showing the reviews (`issues/done/web_review_form.md`, `issues/done/review_in_debt.md`,
  `issues/done/review_in_mcp.md`).
- No outdated review for a file the section did not cite when the review was given (a source file a later run adds
  to the feature): the rewritten section (REV-20) tells it.
