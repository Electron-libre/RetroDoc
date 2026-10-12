# Read a feature's code once for its use cases and its rules

# Goal
With `[pipeline] rules = true` the code of each feature is sent twice: once to the use cases pass, once to
the rules pass, which reads the same files. Together they are about 80% of the input tokens of a run. Cut
that cost without losing use cases or rules, as `business-read` did for the glossary and the entry points
(ADR 0028).

# Findings (project review of 2026-10-12)
* linkding, `deepseek-chat`, `--no-confidence`, 2026-10-12 (`issues/done/business_rules.md`): the run is 1.18M
  tokens in and 242k out; use cases 437k in and 133k out, rules 500k in and 76k out (75 calls).
* `RuleInput` reads "the same files as its use cases" (`FeatureInput`, `docs/crates/pipeline.md`).
* Both prompts start with the brief and the feature, then the parts of their pass, and end with the code
  (`=== path ===` blocks, `use_cases/prompt.rs` and `rules/prompt.rs`): the shared part comes last, so two
  requests share no prefix beyond the brief.
* `Usage` keeps `prompt_tokens` and `completion_tokens` only: tokens a server served from its prompt cache
  are not counted, so the effect of a cache cannot be seen in the recap.
* Precedent: the joint reading of the business files made 36% fewer calls and 33% fewer input tokens on
  linkding (`issues/done/single_read_per_business_file.md`).

# Approach
1. Measure on a saved run, without a call: per feature, the characters of code the two prompts share and
   their share of each prompt.
2. Record the cached tokens the servers report (the field differs between providers: check the docs of
   each) in `Usage` and in the recap; without it candidate (b) cannot be measured.
3. Candidates, to compare on linkding with the benchmark and the rules figures (count, share backed by a
   test or a doc, duplicates):
   (a) one call per feature answers use cases and rules (one reading; a longer answer, and a rule can no
   longer cite the use cases by slug before they exist);
   (b) the code first, identical in both prompts, so the rules call reuses the prefix the use cases call
   left in the server's cache (no change of answers; depends on the provider and on the order and delay of
   the calls);
   (c) the rules read only the lines the steps of the use cases cite, plus the validations around them.
4. Keep what cuts tokens without moving the quality beyond the spread; an ADR if the passes merge.

# Resources
* `crates/retrodoc-pipeline/src/use_cases/prompt.rs`, `rules/prompt.rs`, `rules/input.rs`
* `crates/retrodoc-llm/src/usage.rs`, `crates/retrodoc-pipeline/src/usage_log.rs`
* `issues/done/single_read_per_business_file.md`, ADRs 0021 and 0028
* `issues/rules_deduplication_and_noise.md` (changes the same pass)

# Hints
* Moving the code to the head of a prompt changes every fingerprint and may change the answers: measure
  the quality, not only the tokens.
* A merged call must keep the incremental re-run: today the use cases and the rules of a feature have their
  own fingerprints.
* A local server has no prompt cache between different requests worth counting on: candidate (b) is for
  hosted models.
* Do it after `rules_deduplication_and_noise`, which decides what the rules pass asks.
