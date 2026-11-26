---
id: BNR
status: draft
adrs: []
---

# The judge's rating of the narratives

## Intent

The quality benchmark asks its LLM judge, in batches, whether each use case narrative of a run reads as business
language. One bad rating must not make a benchmark figure that reads as a finding: the rating shows its batches,
and a rating far from the run's own business-language score is marked and kept out of the series' figure.

## Requirements

### Behaviour

- **BNR-1** (deterministic) `retrodoc benchmark` must print, under the judge's rating of the narratives, the rating
  of each batch: its number, how many of its narratives were rated and how many of those read as business language.
- **BNR-2** (deterministic) When the judge's share of narratives in business language and the run's self-rated
  business-language score (the mean of the use cases' `business_language` scores) differ by more than 25 points,
  `retrodoc benchmark` must print next to the judge's figure, and log at `warn`, that it is off the self-rated
  score by that many points and is to be rated again before it is read.
- **BNR-3** (deterministic) `benchmark-table` must leave the runs of BNR-2 out of the mean and range of
  "Narratives in business language (judge)" and say, under the series, how many runs are left out and why; a run
  with no self-rated score or no rated narrative is never left out.

### Contract

- **BNR-4** (deterministic) `judge.yaml` and the run report must keep, for each batch of narratives in the order
  they were sent, how many narratives it held, how many were rated and how many read as business language; a batch
  whose answer gave no usable verdict is kept with none rated, and a file saved before the batches existed loads
  with no batch and its totals unchanged.
- **BNR-5** (deterministic) The mark of BNR-2 and BNR-3 must be computed from the figures the report already
  holds, so that a report saved before this spec is marked by `benchmark-table` and `benchmark --saved-judge`
  with no re-rating.

## Scenarios

- **BNR-H1** (happy; BNR-1, BNR-4) Given a run with 25 narratives, when `retrodoc benchmark --judge` rates them,
  then `judge.yaml` holds two batches (20 and 5 narratives) with their counts, and the summary prints both lines
  under the judge's figure.
- **BNR-H2** (happy; BNR-2, BNR-3) Given a series of 4 runs whose judge's shares are 30%, 93%, 92% and 86% and
  whose self-rated scores are all 98%, when `benchmark-table` reads it, then the judge's narratives row counts 3
  runs (86% to 93%) and a note under the series says 1 run is left out, its rating 68 points off the self-rated
  score.
- (BNR-2) Given a judge's share of 73% and a self-rated score of 98% (25 points), then the figure is not marked;
  at 72% it is.
- (BNR-2) Given a run whose judge rated no narrative, then no mark is printed.
- (BNR-4) Given an answer of the second batch that is unparseable twice, then that batch is saved with 5
  narratives, 0 rated, and the first batch keeps its counts.
- (BNR-5) Given a run report saved before the batches existed, with a judge's share of 30% and a self-rated score
  of 98%, then `benchmark-table` leaves it out of the judge's narratives row.

## Non-goals

- Rating each batch more than once and keeping the agreement: the per-batch counts of BNR-4 and a re-rating of
  saved runs decide first whether one rating holds batch outliers the mark does not catch.
- Whether a narrative is really written in business language: it depends on what the model answers.
- The per-pass replay of the use cases (`pass_replay use-cases`): its table shows the judge's share and the
  self-rated score side by side already.
- A cause of the bad rating of 2026-11-19 (`docs/findings.md` §12): the mark says a rating is off, not why.
