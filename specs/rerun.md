---
id: RER
status: accepted
adrs: [0005, 0037, 0040]
---

# Incremental rerun

## Intent

Running `generate` again on a repository must cost only what changed: every pass reuses the saved result of a
unit whose input is the same, and a run that stopped midway resumes where it stopped. A rerun on an unchanged
repository makes no LLM call and rewrites no file.

## Requirements

### Behaviour

- **RER-1** (deterministic) A pass must not call the LLM for a unit whose input fingerprint is unchanged
  since its saved result; it must reuse that result.
- **RER-2** (deterministic) A changed input must redo the unit it belongs to and no other: a changed entry
  point or file redoes its own feature or use case unit only.
- **RER-5** (deterministic) A run that fails or is killed must keep the units already done, and the next run
  must resume with the others only.
- **RER-12** (deterministic) A feature reused from the saved result must keep the place it had among the
  features of its domain, so that the rendered domain page and index list it at the same place.
- **RER-13** (deterministic) A unit asked again must give a section it writes the slug of the saved section of that
  unit that has the same entry points (the same resources, for a domain), so that the edits and the reviews of the
  section still name it (SED-1, REV-1).

### Contract

- **RER-3** (deterministic) Another model must redo the units it did not answer: the model is part of what is
  compared, for every pass that keeps fingerprints.
- **RER-4** (deterministic) A unit the LLM cleanly answered with nothing (no use case) must be remembered
  until its input changes.
- **RER-6** (deterministic) An artifact must be written beside and renamed over the old one, leaving no
  temporary file.
- **RER-7** (deterministic) `generate --force` must remove the saved results of the LLM passes and keep the
  hand-editable files and the usage history.
- **RER-8** (deterministic) Rendering again unchanged artifacts must rewrite no file, the index included.
- **RER-9** (deterministic) An unparseable answer must not be remembered: the unit must be asked again on the
  next run. An LLM error leaves the unit not done, which RER-5 covers.
- **RER-10** (deterministic) A unit saved under another request must be asked again once, as for another
  model (RER-3). The request of a pass is what it sends besides its input: its system prompt, the JSON schema
  of its answer type and the fixed text of its user prompt (the framing of the product brief included), all
  constants of the binary, so two runs of the same binary on the same input compare equal.
- **RER-11** (deterministic) The rules pass must compare the request of its statement (with the question on
  the kind of feature) apart from that of its judgement: a feature whose statement print is unchanged and whose
  judgement print changed must be judged again from its saved stated rules, with no statement call.
- **RER-14** (deterministic) A section whose entry points are not those of a saved section of its unit, or that has
  none, must keep the slug its answer gives it: the match renames a section, it never merges two.
- **RER-15** (deterministic) A saved slug must not be given back when a section already holds it (in the answer or, for
  a feature, another domain), when the matching section of the answer holds a saved
  slug of its unit, or when another section, of the answer or saved, has the same entry points: each section then
  keeps the slug its answer gives it.
- **RER-16** (deterministic) The domain the code gathers what no domain claimed in (`uncategorized`) must keep its
  slug and give it to no other domain.

## Scenarios

- **RER-H1** (happy; RER-13) Given a feature whose use cases `compute-order-time` (entry point E1) and `compute-rider-time` (E2) were
  generated, and a sibling use case marked wrong, when `generate` asks the feature again and the model answers
  `estimate-order-time` for E1, then the use case is saved as `compute-order-time` and the edit of it still shows.
- (RER-13) Given a domain asked again whose answer renames a feature that groups the same entry points, then the
  feature keeps its saved slug and its use cases are reused.
- (RER-13) Given a feature over two slices of its domain, when the second slice is asked again and renames its part,
  then the part takes the saved slug back and merges with the part of the first slice.
- (RER-13) Given the domains asked again and a domain answered under a new slug with the same resources, then it
  keeps its saved slug.
- (RER-14) Given a saved feature grouping E1 and E2, when the answer splits them into two features, then both keep
  the slugs of the answer.
- (RER-15) Given saved use cases `a` (E1) and `b` (E2), when the answer gives `b` to E1 and `c` to E2, then they stay
  `b` and `c`; when two use cases of the answer both cite E1 alone, then neither takes `a`.
- (RER-1) Given a repository generated once, when `generate` runs again with no change, then no LLM call is
  made and the render plan holds no change.
- (RER-2) Given two features reading `a.rs` and `b.rs`, when `a.rs` changes, then the use cases of the first
  feature are asked again and those of the second are not.
- (RER-10) Given a file read by the glossary pass and saved, when its prompt changes and the file does
  not, then the file is read again on the next run, and the run after makes no call.
- (RER-10) Given a run saved before the prompts were compared, when `generate` runs again, then every
  unit of the reading, domains, features, placement, use cases, actors, reconciliation and confidence passes
  is asked again once (no migration).
- (RER-10) Given a feature saved by the features pass, when a field is added to the answer type of the pass
  and nothing else changes, then the feature is asked again on the next run.
- (RER-10) Given the actors saved, when the fixed text of the actors user prompt changes, then they are asked
  again on the next run.
- (RER-11) Given the rules of a feature stated and judged, when only the judgement prompt changes, then the
  next run makes one judgement call for that feature and no statement call, and the run after makes no call.
- (RER-11) Given a run saved before the judgement had its own print, when `generate` runs again, then the
  rules of every feature are stated again once (no migration).
- (RER-12) Given a domain asked in two slices, the second one giving the features `b` then `c`, and the
  placement putting an entry point of the first slice into `c`, when `generate` runs again with no change,
  then the features of the domain keep the order of the first run and the render plan holds no change.
- (RER-4) Given a feature whose answer is `{"use_cases": []}` twice, when the run is repeated, then it makes
  no call; when a file of the feature changes, then it asks again.

## Non-goals

- The hand-editable artifacts (`roles`, `sources`, `business-files`, `brief`, `contexts`) are not redone when
  their prompt changes: they stay until `--force`, like an edit by hand.
- A real prompt change is not told apart from a cosmetic one: any change of its text redoes the pass.
- No single global "has anything changed" check: one edited file must not redo everything.
- No list of the saved slugs in the prompt: the match is made on the answer, and a section whose entry points
  changed is a new section with the slug its answer gives it.
- The cost of a first run, and the quality of what a pass writes, are other capabilities.
