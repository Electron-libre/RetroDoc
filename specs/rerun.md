---
id: RER
status: draft
adrs: [0005, 0037]
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
- **RER-10** (deterministic) A unit saved under other fixed instructions must be asked again once, as for
  another model (RER-3); the fixed instructions are the system prompts and the fixed text of the user prompts
  of its pass, so two runs of the same binary on the same input compare equal.

## Scenarios

- (RER-1) Given a repository generated once, when `generate` runs again with no change, then no LLM call is
  made and the render plan holds no change.
- (RER-2) Given two features reading `a.rs` and `b.rs`, when `a.rs` changes, then the use cases of the first
  feature are asked again and those of the second are not.
- (RER-10) Given a file read by the glossary pass and saved, when its instructions change and the file does
  not, then the file is read again on the next run, and the run after makes no call.
- (RER-10) Given a run saved before the instructions were compared, when `generate` runs again, then every
  unit of the LLM passes is asked again once (no migration).
- (RER-4) Given a feature whose answer is `{"use_cases": []}` twice, when the run is repeated, then it makes
  no call; when a file of the feature changes, then it asks again.

## Non-goals

- The hand-editable artifacts (`roles`, `sources`, `business-files`, `brief`, `contexts`) are not redone when
  their instructions change: they stay until `--force`, like an edit by hand.
- A change of instructions is not told apart from a cosmetic one: any change of their text redoes the pass.
- No single global "has anything changed" check: one edited file must not redo everything.
- The cost of a first run, and the quality of what a pass writes, are other capabilities.
