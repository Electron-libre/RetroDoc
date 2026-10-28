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
- **BSC-10** (deterministic) When a judge call offered generated and reference items and its answer proposes no
  pair, because it is empty, unparseable or every proposal was dropped, the judge must ask once more and keep the
  pairs of the second answer.
- **BSC-11** (deterministic) The judge must log at `warn` the proposals it drops for naming an item not offered,
  with their count, and must log at `warn`, naming domains or features, a call that still proposes no pair after
  the second answer; a repeated pair must not be warned about.
- **BSC-12** (deterministic) A kind (domains or features) whose call still proposes no pair after the second
  answer must be marked unjudged in `judge.yaml`; a `judge.yaml` saved without the mark must load as judged.
- **BSC-13** (deterministic) A run whose kind is unjudged must have no judged recall and precision of that kind in
  its `run-<n>.json`, so that `benchmark-table` leaves it out of the mean and range of the judged figures of that
  kind, and `benchmark-table` must say, under the series, how many runs are unjudged for each kind.
- **BSC-14** (deterministic) A proposal whose name is an offered name followed by `:` and its description, or the
  start of it, must be read as that offered name; followed by any other text, it must be dropped.
- **BSC-15** (deterministic) A per-pass replay whose kind is unjudged must have no judged recall and precision, show
  its run as unjudged in the judge columns of its table, leave it out of the mean and range of those columns, and
  say under the table how many runs are unjudged; the summary `retrodoc benchmark` prints must show an unjudged
  kind as unjudged instead of its judged recall and precision.

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

- (BSC-14) Given the offered feature "Order clearing" and a proposal named "Order clearing: Removes a customer's
  orders from the router.", then the pair is kept under "Order clearing".
- (BSC-10) Given a first features answer `{"matches":[]}` and a second one with a pair of offered names, then the
  judgement holds that pair and the judge made two features calls.
- (BSC-12) Given two features answers with no pair, then `judge.yaml` marks the features unjudged and the
  `(judged)` feature figures of the run are empty while its strict ones stay.
- (BSC-13) Given a series of 5 runs of which 2 have unjudged features, then the judged feature rows count 3 runs and
  a note under the series says 2 runs are unjudged for features.
- (BSC-15) Given a features replay series of 3 runs of which 1 is unjudged, then the judge columns of that run read
  `unjudged`, their mean and range count the 2 other runs and a note under the table says 1 run is unjudged.
- (BSC-15) Given a run whose features are unjudged, then the printed summary reads, under the judge's proposals,
  that the features are unjudged and gives no judged feature recall or precision; the domains keep theirs.

## Non-goals

- Re-scoring the previous series from its saved clones inside `benchmark-table`: `benchmark --saved-judge --out`
  re-scores one saved clone.
- Whether the judge's pairs are right: it depends on what the model answers, the user reads `judge.yaml` and
  freezes the right pairs in `matches.yaml`.
- Undoing a match by equal names: a pair adds a reference item, it never takes one away (a generated name equal
  to a reference name that means another capability has not been seen).
- Rejecting a judge proposal in `matches.yaml`: a wrong proposal stays out of the hand-written figures only.
- Marking unjudged the runs of a `judge.yaml` saved before BSC-12: they keep their figures; re-judging them is a
  paid run.
- A looser matching of the proposals' names than `normalize` and BSC-14 (a part of a name, a synonym): the
  replay of the issue showed only names copied with their description.
