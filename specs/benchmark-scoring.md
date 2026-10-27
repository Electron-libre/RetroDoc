---
id: BSC
status: accepted
adrs: []
---

# Scoring a run against the benchmark reference

## Intent

The quality benchmark scores the domains and features of a run against a hand-written reference, through the
pairs written by hand in `matches.yaml` and the ones the LLM judge proposes. The recall of an application must not
depend on how the model groups capabilities: a generated item that holds several reference items counts for each. A
change of these rules must not read as a change of quality between two benchmarks.

## Requirements

- **BSC-1** (deterministic) A generated name must cover the reference item of the same normalized name and the
  reference item of every pair that names it; recall must count each reference item covered once.
- **BSC-2** (deterministic) Precision must count a generated name once, whatever the number of reference items it
  covers.
- **BSC-3** (deterministic) A pair that names a reference item absent from the reference must fail the scoring with
  an error naming the pair; a pair whose generated name is absent from the run must be ignored.
- **BSC-4** (deterministic) The judge must be offered every generated item, the ones already paired included, and
  only the reference items that no generated item covers; it must make no call when every reference item is covered
  or nothing was generated.
- **BSC-5** (deterministic) The judge's proposals must keep several reference items for one generated name; a
  proposal naming an item that was not offered, or repeating a pair already proposed, must be dropped.
- **BSC-6** (deterministic) A saved run report must carry the scoring version of the code that scored it; a report
  saved before the version existed must still load, with no version.
- **BSC-7** (deterministic) `benchmark-table --previous` must show the previous figures and the change of the
  recalls, precisions and judge's rating of a series only when every run of both sides carries the same scoring
  version; otherwise it must leave them empty, keep them for the other figures, and say, under the series, which
  versions each side has and that the runs of another version must be re-scored to be compared.
- **BSC-8** (deterministic) `benchmark --saved-judge` must score the run with the judge's proposals and rating
  saved in its `judge.yaml`, with no LLM call, and fail naming the file when there is none.
- **BSC-9** (deterministic) `benchmark-table` must say, under a series, when its runs do not all carry the same
  scoring version, with or without `--previous`.

## Scenarios

- (BSC-1) Given `matches.yaml` pairs "Voting and flagging" with "Upvote stories and comments" and with "Flag stories
  and comments", when the run has "Voting and flagging", then both reference features are covered.
- (BSC-1) Given a generated name equal to a reference name and a pair of that generated name with another reference
  item, then both reference items are covered.
- (BSC-2) Given 4 generated names of which one covers 3 reference items and the others none, then precision is 1/4.
- (BSC-7) Given a previous series saved with no version and a current one at the current version, then no change
  is shown and the note names "none" for the previous side.
- (BSC-7) Given two series that both have no version, then no change is shown: an unknown version is never
  assumed equal.
- (BSC-7) Given a previous series of another version, then the LLM calls and the sizes still show their change.

## Non-goals

- Re-scoring the previous series from its saved clones inside `benchmark-table`: `benchmark --saved-judge --out`
  re-scores one saved clone.
- Whether the judge's pairs are right: it depends on what the model answers, the user reads `judge.yaml` and
  freezes the right pairs in `matches.yaml`.
- Undoing a match by equal names: a pair adds a reference item, it never takes one away (a generated name equal
  to a reference name that means another capability has not been seen).
- Rejecting a judge proposal in `matches.yaml`: a wrong proposal stays out of the hand-written figures only.
