---
priority: low
depends_on: [repeat_the_narratives_rating_of_the_judge]
why: 'the off mark already keeps the worst ratings out of the table; a flipped batch within the bound moves a figure by about 20 points, rarely'
---
# Rate again a batch of narratives the judge flipped to technical

# Goal
The judge rates the narratives in batches of 20. Sometimes it rates a whole batch technical where other ratings of
the same batch find it business language. A rating more than 25 points off the self-rated score is marked and left
out of the table (`specs/benchmark-narrative-rating.md`), but a flipped batch in a rating within the bound still
moves the figure. Expected: a flipped batch is rated again before its verdicts count.

# Findings
* 2026-11-26, the application of `docs/findings.md` §12 (hidden run of `c959836`, 73 narratives in batches of
  20/20/20/13, self-rated 98%), deepseek-chat, five ratings: 84%, 74%, 77%, 60%, 77%. The second batch, the same 20
  narratives each time, read 3 of 20 and 0 of 20 in business language in the 74% and 60% ratings, 15 to 20 of 20 in
  the three others. The other batches stayed within a few narratives of each other.
* The 60% rating is marked (37 points off) and left out; the 74% one (24 points off) is not, and its flipped batch
  takes about 15 points off it.
* A rating costs 4 judge calls for 73 narratives (under a tenth of a cent on DeepSeek).

# Approach
1. Measure first: is a flipped batch always near 0, and how often in more ratings (the saved runs of linkding and
   Lobsters too)? The batches are now saved in `judge.yaml` and the run report.
2. Candidates, to combine: (a) rate again a batch whose share is far below the other batches' and keep the second
   answer, or the higher one; (b) rate each batch twice and a third time when they disagree; (c) smaller batches,
   so a flip weighs less.

# Specs
* `specs/benchmark-narrative-rating.md`: amends its non-goal on rating a batch more than once.

# Milestone
Read by someone taking an application over

# Resources
* `crates/retrodoc-bench/src/benchmark/narratives.rs` (`rate_narratives`, `RATE_SYSTEM_PROMPT`).
* The ratings of 2026-11-26: `repeat_the_narratives_rating_of_the_judge-6873831-rerate/` in the runs directory.
* `issues/done/repeat_the_narratives_rating_of_the_judge.md`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-26

## Understanding
A batch far below the other batches of its rating (more than 50 points under their median) is rated twice more and
counts the middle of its three ratings, so one flipped answer no longer moves the figure. Out of scope: rating
every batch twice, smaller batches (non-goals of the spec). Means: candidate (a) of the issue, with a median of three.

## Plan
1. [x] Measure: five ratings each of the hidden run-1 of linkding and of Lobsters (`dafea18`, session copies,
   ~$0.02): how often a batch flips, whether to near 0, and how far below the others a normal batch reads, to
   check the 50-point bound; verified by the batches of `judge.yaml`; recorded in `docs/findings.md` §12 and the
   ledger. A bound the figures contradict goes back to the user. (autonomous)
2. [ ] Rate again a flipped batch: `rate_narratives` rates a batch twice more past the bound and keeps the middle
   rating, `judge.yaml` and the run report keep each rating of the batch, the summary prints them; verified by
   tests with a fake judge; makes true BNR-6, BNR-7, BNR-8, BNR-9, BNR-H3.
3. [ ] Measure after: five ratings of the hidden copy of `6873831-rerate` at the new code (<$0.01), against the
   84/74/77/60/77% of 2026-11-26; recorded in `docs/findings.md` §12 and the ledger.
4. [ ] Docs of the whole issue and fixes of its `high` review.

## Decisions
- Mode: autonomous
- Base commit: 35e4d4acc3aeb2d98bd12db41d4296103c701aed
- Bound of the re-rating: 50 points below the median of the other batches, as first rated.
- A batch past the bound is rated twice more and keeps the middle of its ratings (not the higher: no upward bias).
- The paid measures of deliverables 1 (~$0.02) and 3 (<$0.01) are agreed.

## Deliverable 1
- done: five ratings each of the hidden run-1 of linkding (85, 90, 85, 89, 81%) and Lobsters (78, 83, 79, 82, 80%)
  at `35e4d4a`, ~$0.02: no batch flipped in 90; technical batches read low in every rating, 40 to 70 points below
  the others (Lobsters batches 10 and 11, linkding's last). The 50-point bound kept: it also catches those
  technical batches, which the middle of three keeps low at two calls each, and a flip reads 82 points or more
  below. Recorded in `docs/findings.md` §12 and the ledger; artifacts in
  `rate_again_a_flipped_batch_of_narratives-35e4d4a-rerate/` of the runs directory. Spec draft committed with it.
- checks: project check passes; no code changed.
- red: none (a measure).
- Code review: no findings
