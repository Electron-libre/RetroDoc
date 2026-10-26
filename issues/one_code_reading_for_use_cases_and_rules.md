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
* `issues/done/rules_deduplication_and_noise.md` (changes the same pass)

# Hints
* Moving the code to the head of a prompt changes every fingerprint and may change the answers: measure
  the quality, not only the tokens.
* A merged call must keep the incremental re-run: today the use cases and the rules of a feature have their
  own fingerprints.
* A local server has no prompt cache between different requests worth counting on: candidate (b) is for
  hosted models.
* Do it after `rules_deduplication_and_noise`, which decides what the rules pass asks.

# Specs
* `specs/token-accounting.md` (TOK-1 to TOK-5)

# Tracking
1. [x] Cached prompt tokens in `Usage` and in the totals (TOK-1, TOK-2, TOK-3)
2. [x] Cached tokens in the recap and the usage history (TOK-4, TOK-5); spec `accepted` then `implemented`
3. [x] Offline measure of the code shared by the use cases and rules prompts, on the saved linkding run
4. [x] Candidate (b) - dropped: code first in both prompts; linkding run on DeepSeek, cached tokens and benchmark
5. [ ] Candidate (c): rules read only the lines cited by the use cases; linkding measure
6. [ ] Candidate (a): one call per feature (only if (b) and (c) fall short; needs an ADR)

## Decisions
* Mode: autonomous. Stops before each paid DeepSeek run and at each decision on a candidate.
* Order: 1, 2, 3, then the candidates (b), (c), (a); (a) may never be done.
* A candidate that moves quality beyond the spread is dropped and noted here.
* Deliverables 1 and 2 were delivered together: the struct change broke the compilation of `usage_log.rs`. Spec `token-accounting` is `implemented`.
* Per-pass cached column is shown only when the run has cached tokens, so the recap is unchanged otherwise.
* Code review of 1+2: fixed a zero `cached_tokens` hiding `prompt_cache_hit_tokens` (max of the two). Not fixed, by design: the cap at the prompt is TOK-3, pricing is a non-goal of the spec, the "cached" column is shown only when the run has cache hits.

## Measure of step 1 (deliverable 3, `examples/code_read_overlap.rs`)
linkding, frozen inputs, 32 features, both passes replayed against a recording provider (no call), first
prompt that shows code per feature:
* The code (`=== path ===` blocks) is 89% of a use cases prompt and 92% of a rules prompt (937k and 907k
  characters). The two prompts show the same excerpts of the same files: 833k characters, 89% / 92%, are
  shared, 45% of the 1.84M characters the two passes send.
* So a single reading (candidate (a)) is worth up to 45% of the input of the two passes. Candidate (b) is
  bounded by the same figure but only if the code comes first (a common prefix, not counted here) and the
  calls come close in time. The other rules calls (kind question, judgement) carry no code. A statement
  call cut at the completion limit is asked again on halves, which the replay does not see: the figures
  are a floor for such a feature. The brief and the feature lines before the code differ in size per
  prompt (~3k characters): they break the common prefix today.
* (c) can cut at most the 92% of the rules prompt that is code, and only as far as the cited lines allow.

* Code review of 3: fixed the pass detection by content (one recorder per pass); documented the limits
  (first prompt with code only, `=== x ===` in a source). Not fixed: features keyed by name (all unique on
  linkding), trailer after the code (none: the code is last in both prompts).

## Candidate (b), first run (deliverable 4, in progress)
`src/cache_first.rs`, active with `RETRODOC_CODE_FIRST=1` (experiment, not wired for users): one fixed system
sentence, then in the user message the brief, the code (`=== [Fn] path ===` in both passes), the task (the
former system message), the feature and the parts of the pass. The system message must be shared too: a
cache is a prefix of the whole request, system message included.
* linkding, `deepseek-chat`, frozen inputs, use cases then rules, one run each: use cases 11.9k of 282.8k
  prompt tokens cached (cold); rules **244.1k of 291.4k (84%)**. DeepSeek serves the prefix the use cases
  call left, across the whole pass. Runs saved in `~/retrodoc-runs/code-first/`.
* Quality, linkding, `deepseek-chat`, frozen inputs (files `base-*` and `cf-*` in `~/retrodoc-runs/code-first/`):
  use cases, 3 runs each: 80 (78-81) vs 78 (75-79) use cases, business language by the judge 87% vs 84%
  (77-89), score = 1 64% vs 74%: no move beyond the spread. Rules, 7 runs each: kept 270 (256-280) vs 237
  (214-256), reference rules stated 11.4 vs 8.6 of 29 (base 9-13, code first 6-10; the 4 last runs of each
  do not overlap: 12-13 vs 8-10). The rules lose about 12% of their rules and a quarter of the reference
  ones.
* **Dropped**: the prompt tokens of the tables do not change (about 281k per pass in both layouts), only their price does (84% served from the cache on
  the rules pass after a cold use cases pass), and the rules pass loses quality. The cause is probably the
  task moved after the code, into the user message; a layout that keeps the system message of each pass
  has no common prefix. Repeated runs of one layout hit their own cache (about 95%), so only the cold
  use cases then rules run measures the cross-pass reuse. The experiment code (`cache_first.rs`, three
  call sites) is removed; `pass_replay` now prints the cached tokens of each run.
* Code review of the drop: not fixed, by design: `surface_pass` (no code, no cache question) and the two-line
  duplicate in the example; the evidence stays in `~/retrodoc-runs/code-first/` as for the other measures.
