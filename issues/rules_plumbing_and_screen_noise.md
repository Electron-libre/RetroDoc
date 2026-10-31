---
rank: 900
why: 'low (user, 2026-10-23): fine tuning of rules that are already usable; each step gained less than the one before, so after `contexts_residual_size_and_names` and well behind the structural work'
---
# Lower the plumbing and screen behaviour left among the business rules

# Goal
After the judgement per rule and the set-aside of the features about tests, about a third of the rules of a
large public repository are still plumbing or screen behaviour. Find out whether that is worth more work, and
what moves it, without losing business rules. Not a priority: the rules are usable, and each step so far
gained less than the one before.

# Findings (smoke test on linkding, local `qwen3.6:35b-a3b`, 2026-10-23)
* Hand labels of 60 rules after the pass: 63% good, 22% plumbing (a default date set to now, a folder skipped
  if missing, a tag cache), 13% screen behaviour, 2% duplicates, 0% test. On a large application with
  DeepSeek: 68% good, 28% plumbing, 5% screen.
* The judgement per rule dropped 29% of the statements of one run, but its effect from a run to another is not
  steady: replayed on a saved set, a closed class per rule lost 3% of the good rules and removed 65% of the
  noise; end to end the share of noise did not move until the features about tests were asked apart.
* The rules a feature states vary a lot from a run to another (the same feature from 8 to 32 rules), so a
  change of share is hard to read on one run.

# Approach
1. Measure first on a fixed set of statements (replay the judgement on saved `rules-stated.yaml` of several
   runs and repositories), not on a new run each time, to separate the effect of a prompt from the variance
   of the statement pass. The saved runs that hold one (linkding, `delivery_router`, a Django application, the
   large Rails application) are listed in the planned measures of the ledger (`CLAUDE.md`).
2. Candidates: (a) examples of what is not a rule in the statement prompt (a first sentence on storage did not
   move the count on a small repository); (b) a second class question with the neighbours of the rule (same
   file, same feature); (c) lower the rank of the rules backed by code alone in the rendered pages.
3. Keep only what lowers the share of plumbing without raising the good rules dropped above 3%.

# Milestone
Business rules quality

# Resources
* `issues/done/rules_deduplication_and_noise.md` (figures, replay method)
* `crates/retrodoc-pipeline/src/rules/` (`judge.rs`, `prompt.rs`)

# Hints
* No pattern of a language or a framework to recognize plumbing: the LLM judges, the checks are mechanical.
* Low priority (user, 2026-10-23): fine tuning of a result that is already usable.

# Tracking

Plan: validated

## Understanding
Tell, on fixed statements, whether a change of the judgement lowers the plumbing and screen rules it keeps
without dropping more than 3% of the good ones; adopt it only then, else close on the measure. Out of scope: a
pattern of a language or framework. Means: the saved `rules-stated.yaml` hold the rules the current judgement
*kept* (its drops are not saved before `rules-set-aside.yaml` kept them whole), so a replay measures what a
variant drops *on top of* today's judgement, which is the residual noise the issue is about.

## Plan
1. [x] Judgement replay example (`rules_judge_replay`): loads `features.yaml` and `rules-stated.yaml` of saved
   runs, asks the judgement of each feature with a chosen variant N times, prints per run the rules, the drops
   per class and, given a labels file (`<rule slug>\t<good|plumbing|screen|test|duplicate>`), the good rules
   dropped and the noise dropped; keeps the verdicts. The judgement is reached through a `benchmark` entry that
   takes the system prompt, so the pipeline and the replay share `judge_prompt` and `keep_judged`; verified by a
   fake-LLM test of that entry (classes counted, unknown class kept); about 300 lines.
2. [ ] Labels and baseline (no code): hand labels of a sample of the kept rules (linkding
   `retrodoc-smoke-linkding` 60, `rsa-hc` 40, the large Rails application 40, `rsa-dr` all 77) in
   `~/retrodoc-runs/rules_plumbing-<commit>/labels/`; 3 replays of today's prompt on the 7 runs of the ledger
   (DeepSeek, cents) give the drop on top of today and its spread; findings and ledger line.
3. [ ] Variants of the judgement, same measure: (a') examples of what is not a rule (plumbing, screen) in the
   judgement prompt; (b) a second class question on the rules kept `business`, with the other rules citing the
   same file. The one that lowers the noise kept and drops at most 3% of the good rules goes into
   `rules/judge.rs` with its fake-LLM test (and a spec of the judgement if (b), a new call of the pass); none:
   findings only, the issue closes on the measure.

## Deliverable 1
- done: `benchmark::replay_judgement` and `score_labels` (the judgement shared with the pass through
  `rules::judge_rules`), example `rules_judge_replay`.
- red: none (no spec): `each_rule_gets_the_class_of_the_answer_of_its_feature` failed on a stub body (`left: 0,
  right: 1` on the unread count).
- not verified: the example on a real run (deliverable 2 does it).

## Decisions
- Mode: autonomous
- Base commit: dcb69ef
- Paid runs: about $0.5 of DeepSeek for deliverables 2 and 3 (user, 2026-10-31).
- Candidate (a) on the statement prompt left out: not measurable on fixed statements; (a') carries its examples into the judgement.
- Candidate (c) left out: it moves the noise down a page, it does not lower its share.
- Labels by the assistant, sample kept in `~/retrodoc-runs`.
