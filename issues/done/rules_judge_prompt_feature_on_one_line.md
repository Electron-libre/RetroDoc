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

# Tracking

Plan: validated

## Understanding
Put the feature name and description on one line (`one_line`) in `judge_prompt`, so a multi-line description can't
forge a numbered rule. The statement prompt (`rules_prompt`, `prompt.rs`) has the same gap (a description line
could forge an evidence id `[T1]`, `[D1]`): fixed too, same two lines. The reconciliation already uses `one_line`.
No spec: the prompt shape is internal (no spec covers the judgement); the prints hash the raw fields, not the
prompt, so no saved run is asked again.

## Plan
1. [x] Feature on one line in the judgement and statement prompts: `one_line` on name and description in
   `judge_prompt` and `rules_prompt`; verified by one test of both prompts with a description over several lines (the
   judgement keeps its numbering, the statement prompt forges no evidence line).
2. [x] Fixes of the `high` review: the feature line shared by every prompt that shows it (`text::feature_line`,
   the use case prompt included), one `one_line` for the pipeline (`text.rs`, the copies of `reconcile/` and
   `report/` removed), the use case names of the statement prompt on one line; the tests find each prompt by its
   system prompt; verified by the test of the rules prompts, extended, and one of the use case prompt.

## Summary of deliverable 1
done: `one_line` on the feature name and description in `judge_prompt` and `rules_prompt`.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean |
| specs | n/a (no spec of the judgement prompt) |
| docs | n/a (`docs/crates/pipeline.md` does not describe the prompt lines) |
| code review | `low`, clean |

red: none traced (no spec); before the code the test failed on the statement prompt, whose description was not on one line.

Code review: no findings

Not verified: none. Decisions needed: none.

## Summary of deliverable 2
done: `text::feature_line` and `text::one_line` shared; the use case prompt and the use case names of the
statement prompt on one line; the rules test reads the prompts by system prompt and checks both numbered rules.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean |
| specs | n/a |
| docs | n/a (no page describes the prompt lines or `text.rs`) |
| code review | `low`, clean |

red: none traced (no spec); before the code the rules test failed on the use case name off its line.

Code review: no findings

Not verified: none. Decisions needed: none.

Findings of the `high` review not fixed here:
- Saved runs are not asked again (the prints hash the raw fields): as the Approach chose, no forging was seen on
  a run and re-asking every feature would cost a paid run.
- The entities and vocabulary lists, and the render and MCP copies of `one_line`, are left to
  `issues/done/one_line_at_the_source.md`.

## Decisions
- Mode: autonomous
- Question 1: a, `rules_prompt` is fixed in the same deliverable.
- Base commit: af3734f0ebacc11bf57695c35ec320e25a1f1d48
