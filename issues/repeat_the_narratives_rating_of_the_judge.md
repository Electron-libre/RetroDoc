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
