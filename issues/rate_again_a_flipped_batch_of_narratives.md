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
* `issues/repeat_the_narratives_rating_of_the_judge.md`.
