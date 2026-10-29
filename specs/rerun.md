---
id: RER
status: draft
adrs: [0005]
---

# Incremental rerun

## Intent

Running `generate` again on a repository must cost only what changed: every pass reuses the saved result of a
unit whose input is the same, and a run that stopped midway resumes where it stopped. A rerun on an unchanged
repository makes no LLM call and rewrites no file.

## Requirements

- **RER-1** (deterministic) A pass must not call the LLM for a unit whose input fingerprint is unchanged
  since its saved result; it must reuse that result.
- **RER-2** (deterministic) A changed input must redo the unit it belongs to and no other: a changed entry
  point or file redoes its own feature or use case unit only.
- **RER-3** (deterministic) Another model must redo the units it did not answer: the model is part of what is
  compared, for every pass that keeps fingerprints.
- **RER-4** (deterministic) A unit the LLM cleanly answered with nothing (no use case) must be remembered
  until its input changes.
- **RER-5** (deterministic) A run that fails or is killed must keep the units already done, and the next run
  must resume with the others only.
- **RER-6** (deterministic) An artifact must be written beside and renamed over the old one, leaving no
  temporary file.
- **RER-7** (deterministic) `generate --force` must remove the saved results of the LLM passes and keep the
  hand-editable files and the usage history.
- **RER-8** (deterministic) Rendering again unchanged artifacts must rewrite no file, the index included.
- **RER-9** (deterministic) An unparseable answer must not be remembered: the unit must be asked again on the
  next run. An LLM error leaves the unit not done, which RER-5 covers.

## Scenarios

- (RER-1) Given a repository generated once, when `generate` runs again with no change, then no LLM call is
  made and the render plan holds no change.
- (RER-2) Given two features reading `a.rs` and `b.rs`, when `a.rs` changes, then the use cases of the first
  feature are asked again and those of the second are not.
- (RER-4) Given a feature whose answer is `{"use_cases": []}` twice, when the run is repeated, then it makes
  no call; when a file of the feature changes, then it asks again.

## Non-goals

- Changing a prompt does not invalidate the saved results by itself (`--force` does, ADR 0005).
- No single global "has anything changed" check: one edited file must not redo everything.
- The cost of a first run, and the quality of what a pass writes, are other capabilities.
