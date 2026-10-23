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

# Tracking
1. [x] Measure, without an LLM call, the rules and the answer length per feature on the saved runs
   (`rules-ld-*`, `rules-dr-*`, `rules-contrat-*`); the figures are in `## Measure` below. They are a lower
   bound and do not decide: go for (b)+(a).
2. [x] Shorter answer per rule (b): the model cites code as `"F1:10-24"` (files numbered in the prompt) and no
   longer writes `entities` (derived from the statement); `use_cases` and the test phrase stay in the answer.
   The real gain in characters and the quality of the citations are only measured at step 4.
3. [ ] Split a large feature into several calls (a), by group of files the feature already holds, merged in a
   stable order (slugs `<feature-slug>-<n>`, dedup after the merge); fall back to two halves when an answer
   ends with `finish_reason=length` (the provider must report the truncation). Reduced to the fallback alone
   if the measure shows no feature near the cap after (b).
4. [ ] Smoke test on linkding with DeepSeek: no feature lost, no truncation warning, second run with no LLM
   call. Ask before launching (paid external API).

## Decisions
* Measure first; the choice between (b) alone and (b)+(a) follows its figures.
* Split upstream on the expected answer size, with a `finish_reason=length` fallback as a safety net.
* The final smoke test uses DeepSeek, not the local model.
* After the measure: go for (b)+(a); log the raw rules and the characters per answer in step 2.
* Step 2 changes the statement prompt, which is now part of the feature fingerprint: saved features are asked
  again once.
* Candidates (c) (cap on the number of rules) and (d) (raise the cap) are dropped.

## Measure (step 1, `examples/rules_answer_size_measure.rs`)
The answer of a feature is rebuilt from its saved rules, so the figures below are a **lower bound**: the cut
answers were never saved, and the rules dropped by grounding, deduplication and the judgement are missing.
* linkding (smoke run): the largest feature, `search-and-filtering`, keeps 38 rules, about 13k characters
  rebuilt (51 % of the cap), yet its real answer was cut at 25.8k characters. So the model writes about twice
  as many rules as survive, and the saved rules cannot tell how big a real answer is.
* Mean rebuilt answer: 373 to 419 characters per rule (statement 20 to 30 %, cited code 20 to 36 %, tests 0 to
  17 %, entities and use cases 2 to 21 %). The code field is the second largest share.
* Contrat (DeepSeek): 3 features of 85 at 50 % or more, one at 84 % (`contract-event-notifications`, 51 rules
  kept); none cut, DeepSeek having a larger window than the local server.
* Cut answers in the logs: 25.5k to 25.8k characters and 1184 to 1315 lines, against 12 rules kept for one
  of them in another run: the answers carry far more rules than the 241 published.
* Conclusion: the rebuilt size is not a sound measure of what is asked. To decide between (b) alone and
  (b)+(a), measure the answers themselves (`answer_chars` in the logs, or log the number of raw rules per
  answer).
