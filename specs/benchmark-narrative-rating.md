---
id: BNR
status: accepted
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
  rounded to whole points,
  `retrodoc benchmark` must print next to the judge's figure, and log at `warn`, that it is off the self-rated
  score by that many points and is to be rated again before it is read.
- **BNR-3** (deterministic) `benchmark-table` must leave the runs of BNR-2 out of the mean and range of
  "Narratives in business language (judge)", the previous series' runs too, and name, under the series, the runs
  left out with their points; a run with no self-rated score or no rated narrative is never left out.
- **BNR-6** (deterministic) When the share of a batch's narratives in business language is more than 50 points
  below the median share of the other batches of the same rating as first rated, rounded to whole points, the
  judge must rate that batch twice more.
- **BNR-7** (deterministic) A batch rated again, and the totals of its rating, must count the rating whose share
  is the middle one of the batch's ratings that gave a verdict (the higher of two; between equal shares, the
  earlier rating).
- **BNR-8** (deterministic) `retrodoc benchmark` must print, on the line of a batch rated again, how many
  narratives each of its ratings found in business language, in the order they were made.

### Contract

- **BNR-4** (deterministic) `judge.yaml` and the run report must keep, for each batch of narratives in the order
  they were sent, how many narratives it held, how many were rated and how many read as business language; a batch
  whose answer gave no usable verdict is kept with none rated, and a file saved before the batches existed loads
  with no batch and its totals unchanged.
- **BNR-5** (deterministic) The mark of BNR-2 and BNR-3 must be computed from the figures the report already
  holds, so that a report saved before this spec is marked by `benchmark-table` and `benchmark --saved-judge`
  with no re-rating.
- **BNR-9** (deterministic) `judge.yaml` and the run report must keep, for a batch rated again, the counts
  (rated, business) of each of its ratings in the order they were made, the first included, next to the counts it
  kept; a batch rated once keeps none, and a file saved before this requirement loads with none.

## Scenarios

- **BNR-H1** (happy; BNR-1, BNR-4) Given a run with 25 narratives, when `retrodoc benchmark --judge` rates them,
  then `judge.yaml` holds two batches (20 and 5 narratives) with their counts, and the summary prints both lines
  under the judge's figure.
- **BNR-H2** (happy; BNR-2, BNR-3) Given a series of 4 runs whose judge's shares are 30%, 93%, 92% and 86% and
  whose self-rated scores are all 98%, when `benchmark-table` reads it, then the judge's narratives row counts 3
  runs (86% to 93%) and a note under the series names the run left out, its rating 68 points off the self-rated
  score.
- **BNR-H3** (happy; BNR-6, BNR-7, BNR-8, BNR-9) Given a run of 73 narratives whose batches the judge first rates
  19 of 20, 0 of 20, 17 of 20 and 8 of 13 in business language, and whose second batch it then rates 17 and 15
  of 20, when `retrodoc benchmark --judge` rates them, then the second batch keeps 15 of 20, the rating counts
  59 of 73, and the summary line of that batch shows 0, 17 and 15.
- (BNR-6) Given batches first rated 90%, 100% and 45% (50 points below the median of the others, 95%), then no
  batch is rated again; at 44% the third is.
- (BNR-6) Given a rating of a single batch, then it is never rated again.
- (BNR-7) Given a batch rated again whose first re-rating gives no usable verdict and whose second reads 16 of
  20, then the batch keeps 16 of 20 (the higher of 0 and 16), and the unusable rating is saved with none rated.
- (BNR-7) Given a batch whose re-ratings read 1 and 2 of 20 after a first 0 of 20, then it keeps 1 of 20: a
  batch that is technical stays so.
- (BNR-6) Given a batch whose first answer gave no usable verdict, then it is not rated again (BNR-4 keeps it
  with none rated).
- (BNR-2) Given a judge's share of 73% and a self-rated score of 98% (25 points), then the figure is not marked;
  at 72% it is.
- (BNR-2) Given a run whose judge rated no narrative, then no mark is printed.
- (BNR-4) Given an answer of the second batch that is unparseable twice, then that batch is saved with 5
  narratives, 0 rated, and the first batch keeps its counts.
- (BNR-5) Given a run report saved before the batches existed, with a judge's share of 30% and a self-rated score
  of 98%, then `benchmark-table` leaves it out of the judge's narratives row.

## Non-goals

- Rating every batch more than once: only a batch far below the other batches of its rating is rated again; a
  rating whose every batch reads low is caught by the mark of BNR-2, not by BNR-6.
- Smaller batches: they would change every judge's figure saved so far.
- An unbiased figure for a technical batch: a batch is rated again only when its first rating is low, so a
  technical batch whose first rating falls past the bound keeps a middle rating no lower than it; a batch read
  too high is never rated again.
- A new scoring version: the re-rating corrects an answer of the judge, and a previous series may still hold a
  flip within the bound of BNR-2.
- Whether a narrative is really written in business language: it depends on what the model answers.
- The per-pass replay of the use cases (`pass_replay use-cases`): its table shows the judge's share and the
  self-rated score side by side already.
- A cause of the bad rating of 2026-11-19 (`docs/findings.md` §12): the mark says a rating is off, not why.
