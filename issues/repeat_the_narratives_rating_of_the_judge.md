---
priority: medium
why: 'one rating of the narratives was 60 points off and would have led to a wrong finding; a guard is cheap'
---
# Keep one bad rating of the narratives from making a benchmark figure

# Goal
The judge rates the business language of the narratives once per run, in batches of 20. On Vingilot one rating gave
30% where three more gave 86% to 93% on the same narratives. A single bad rating makes a figure that reads as a
finding. Expected: a narratives figure that shows when it cannot be trusted, or that one outlier cannot move.

# Findings
* 2026-11-19, Vingilot hidden run of `c959836`, deepseek-chat: ratings 30% (22 of 73), then 93%, 92%, 86%; the
  self-rated business score was 98%; the shown run: 84%, then 80%, 84%, 84% (`docs/findings.md` §12).
* The log of the 30% run shows the server refusing the JSON schema of a call and the judge proposing no domain pair
  twice: the cause of the low rating is not known.

# Approach
1. Measure first: the ratings per batch of the bad run are not saved; save them (batch, verdicts), then look for the
   pattern of a bad rating (one batch all false? all batches low?) on saved runs, re-rating a few times.
2. Candidates, to combine: (a) warn when the judge's share and the self-rated business score differ by more than a
   bound; (b) rate each batch twice and keep the agreement, or a third rating on disagreement; (c) show the rating
   per batch in the report.

# Milestone
Read by someone taking an application over

# Resources
* `crates/retrodoc-bench/src/benchmark/judge.rs` (`rate_narratives`, `RATE_SYSTEM_PROMPT`).
* Copies of the runs re-rated: `benchmark_on_an_application_the_user_knows-c959836-rejudge/` in the runs directory.
* `issues/done/benchmark_spread_and_judge_bias.md`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-26

## Understanding
Make one bad judge rating of the narratives visible and unable to move a series figure: save and print the rating
per batch, mark a rating more than 25 points off the run's self-rated business-language score, and leave the marked
runs out of the table's mean. Rating each batch twice waits for the measure of deliverable 4 (non-goal of the spec).
Spec: `specs/benchmark-narrative-rating.md` (BNR-1 to BNR-5).

## Plan
1. [x] Move the rating of the narratives (`NarrativeRating`, `rate_narratives`, its prompt and tests) out of
   `judge.rs` (1,460 lines) into `benchmark/narratives.rs`, no change of behaviour; verified by the existing tests;
   makes room for the next two (~300 lines). (autonomous)
2. [x] Batches kept and printed: `judge.yaml` and the run report keep each batch's counts, `benchmark` prints them;
   verified by tests of the rating, the load of an old `judge.yaml` and the summary; makes true BNR-1, BNR-4, BNR-H1
   (~250 lines). (autonomous)
3. [x] Off rating marked: the summary marks and warns of a share more than 25 points off the self-rated score, the
   table leaves those runs out of the judge's narratives row with a note, computed from the saved figures; verified
   by summary and table tests, one on a report saved before the batches; makes true BNR-2, BNR-3, BNR-5, BNR-H2;
   spec `accepted` (~300 lines). (autonomous)
4. [x] Measure (paid, ~$0.01: 8 judge calls per re-rating): `retrodoc benchmark --judge` 5 times on a copy of the
   Vingilot hidden run of `c959836` with the batches saved, to see whether a bad rating is one batch or every batch;
   recorded in `docs/findings.md` §12 and the ledger; a follow-up issue to rate each batch twice if the batches
   spread. (autonomous)
5. [x] Docs of the whole issue and fixes of its `high` review. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 2aca8dc70fa79b514d457a3e6e68771c4a35d6b4
- Bound of the mark: 25 points (normal gap -3 to 18 points on the saved runs, outliers from 46).
- A marked run is left out of the mean and range of the judge's narratives row, as an unjudged kind is (BSC-13).
- The paid measure of deliverable 4 is agreed (~$0.01).

## Deliverable 1
- done: the rating of the narratives moved from `judge.rs` to `benchmark/narratives.rs`, re-exported from
  `benchmark`; no change of behaviour; spec draft `benchmark-narrative-rating` committed with it.
- checks: tests pass, clippy clean, specs check pass, docs (`docs/crates/bench.md`) updated.
- red: none (a move, the existing tests unchanged).
- Code review: no findings

## Deliverable 2
- done: `NarrativeRating` keeps a `BatchRating` per batch (narratives sent, rated, business), saved in `judge.yaml`
  and the run report (absent from an old file, which loads with none); `retrodoc benchmark` prints a line per batch
  under the judge's figure.
- checks: tests pass, clippy clean, specs check pass, docs (`docs/crates/bench.md`) updated.
- red: BNR-4, BNR-1, BNR-H1: `cannot find struct BatchRating`, `NarrativeRating has no field named batches`.
- Code review: no findings

## Deliverable 3
- done: `NarrativeRating::off_by` gives the points by which the judge's share is more than 25 points off the
  self-rated score; `retrodoc benchmark` prints the mark under the judge's figure and warns; `benchmark-table`
  leaves those runs out of the judge's narratives row and names them under the series, from the saved figures (an
  old report is marked too). Spec `benchmark-narrative-rating` accepted.
- checks: tests pass, clippy clean, specs check pass, docs (`docs/crates/bench.md`, skill `quality-benchmark`)
  updated.
- red: BNR-2, BNR-3, BNR-5, BNR-H2: `no method named off_by`; then the table row read `75% | 30%–93% | 4` for
  `90% | 86%–93% | 3`, the old report `30% | — | 1` for `n/a | — | 0`, the summary had no mark.
- Code review: no findings
- note: the table test fixture's self-rated score moved from 50% to 90%, its judge's 90% being off otherwise.

## Deliverable 4
- done: five ratings of the hidden copy at `6873831` (<$0.01): 84%, 74%, 77%, 60%, 77%; the low ones are the
  second batch flipped to technical (3/20, 0/20; 15 to 20/20 otherwise); the 60% one marked and left out by the
  table (78%, 74% to 84%, over 4 runs). Recorded in `docs/findings.md` §12 and the ledger; artifacts kept in
  `repeat_the_narratives_rating_of_the_judge-6873831-rerate/` of the runs directory. The batches spread: follow-up
  issue `rate_again_a_flipped_batch_of_narratives` (low, after this one), attached to the milestone.
- checks: project check passes; no code changed.
- red: none (a measure).
- Code review: no findings
- not verified: whether a flipped batch shows on linkding or Lobsters (left to the follow-up's measure).

## Deliverable 5
- done: docs of the whole issue (`docs/ARCHITECTURE.md`, `docs/crates/render-mcp-cli.md`); fixes of the `high`
  review: the table names the runs left out (`Series::files`), says the previous runs are left out too, and the
  hint says `--judge` proposes the pairs again; BNR-2 says the points are rounded; a round trip of `judge.yaml`
  with its batches tested.
- checks: tests pass, clippy clean, specs check pass, docs updated.
- red: none (review fixes; the new tests pass on their first run: the save already kept the batches).
- High review: 6 findings: run not named (fixed), rounding before the bound (spec reworded, the rounding kept so
  the printed points and the bound agree), previous runs left out silently (note says so), `--judge` redoes the
  pairs (hint and skill say so), self-rated score over other use cases or a later run (not fixed: every run has a
  narrative per use case so far, and `--saved-judge` after a new `generate` misreads every judged figure, not
  only this one), BNR-H1 not proving `judge.yaml` (round-trip test added).
- Code review: no findings
