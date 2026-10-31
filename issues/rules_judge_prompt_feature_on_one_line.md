---
rank: 975
why: 'low (high review of `rules_plumbing_and_screen_noise`, 2026-10-31): a robustness fix of a few lines, not seen on a run; right after the rules issues, before the search and print work'
---
# Put the feature on one line in the rules judgement prompt

# Goal
The judgement of the rules numbers the rules of a feature in its user prompt and puts each statement on one line
(`one_line`), so that a statement can't forge a numbered line. The head of the same prompt writes the name and
description of the feature as they are: a description over several lines could forge a numbered rule, shift the
numbers the answer gives and drop the wrong rule. The feature question of the same module already puts them on
one line. Make the judgement prompt do the same.

# Findings (high review of `issues/done/rules_plumbing_and_screen_noise.md`, 2026-10-31, read in the code; not seen on a run)
* `judge_prompt` (`crates/retrodoc-pipeline/src/rules/judge.rs`) writes `Feature: {name} — {description}`
  with no `one_line`; `feature_prompt` in the same file uses it.
* The descriptions come from the features pass and can be edited in `features.yaml`.

# Approach
1. Apply `one_line` to the name and description in `judge_prompt`, with a test of a description over several
   lines that keeps the numbering.
2. The user prompt is not in the print of a feature: no saved run is asked again.

# Milestone
Business rules quality

# Resources
* `crates/retrodoc-pipeline/src/rules/judge.rs`, `rules/tests.rs`

# Hints
* Check the other prompts that number items for the same gap (the statement prompt of the rules, the
  reconciliation) only if the fix stays small; otherwise note them.
