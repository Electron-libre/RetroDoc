# Keep the rules answer of a large feature under the completion limit

# Goal
The rules pass asks one call per feature. On a feature with many rules the JSON answer reaches the
completion cap (`MAX_COMPLETION_TOKENS` = 8192 tokens, about 25k characters), the server cuts it
(`finish_reason=length`), the JSON is unparseable and the feature can lose all its rules. A run must not lose
the rules of its largest features, and the rerun must stay a no-op.

# Findings (smoke test on linkding, local `qwen3.6:35b-a3b`, 2026-10-16)
* 3 truncated answers in the first run (`rules of search-and-filtering`, retried; `rules of
  user-profile-and-application-settings-management`, retried then skipped), one more in the second run.
  The cut answers are 25.5k to 25.8k characters long, which is the 8192 token cap.
* `search-and-filtering` states 38 rules in the end; the skipped feature has no rules in `rules.yaml`.
* The rerun asked 2 LLM calls (the skipped feature is retried on the next run, by design), so the smoke
  test fails "second run is not a no-op" and "no warning": 4 FAIL lines in total, one cause.
* With DeepSeek (`issues/done/business_rules.md`) the unparseable answers were of another kind (wrong
  shapes); whether the cap was reached there was not looked at.

# Approach
1. Measure first: the number of rules and the answer length per feature in a saved run, to see how many
   features come close to the cap.
2. Candidates, can be combined: (a) ask the rules of a large feature in several calls, by group of files or of
   entry points, and merge the results (slugs stay `<feature-slug>-<n>`); (b) ask for a shorter answer per
   rule (the statement and the cited lines only), the evidence the code already gives being added by the
   program; (c) tell the model the maximum number of rules and keep the most useful ones; (d) raise the cap
   for this pass only, if the server accepts it (a hosted model has a larger limit).
3. Re-run the linkding smoke test: it must be conclusive and lose no feature.

# Resources
* `crates/retrodoc-llm/src/openrouter.rs` (`MAX_COMPLETION_TOKENS`, the `finish_reason=length` warning)
* `crates/retrodoc-pipeline/src/rules/` (`mod.rs`, `prompt.rs`)
* `issues/shorter_use_case_answers.md` (same question for the use cases pass)
* `.claude/skills/smoke-test/SKILL.md`

# Hints
* No pattern of any language or framework to split a feature: group by the files and entry points the
  feature already holds.
* Splitting a feature changes what `rules_deduplication_and_noise` has to compare: do it after or with it.
