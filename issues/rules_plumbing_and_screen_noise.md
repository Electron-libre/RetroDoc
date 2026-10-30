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
