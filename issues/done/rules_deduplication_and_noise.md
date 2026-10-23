# Remove duplicate and plumbing business rules

# Goal
Make the rules pass (`issues/done/business_rules.md`, ADR 0021) state each rule once and keep only those a
business reader would ask about. Today the same rule is stated by several features, and storage and
retrieval plumbing is stated as rules.

# Findings (smoke test on `delivery_router`, local `qwen3.6:35b-a3b`, 2026-10-12)
* 28 rules for 9 features. The 60-minute limit is stated twice and the delivery time formula three times,
  once per feature that reads the same code: each feature is asked on its own and the rules are numbered
  per feature (`<feature-slug>-<n>`), so nothing compares them.
* Plumbing appears as rules: "an order is stored by the customer id", "a rider is retrieved by id",
  "orders can be cleared for a customer", "a rider requires a speed to be initialized".
* One sentence added to the prompt asking to leave out how data is stored, fetched, listed or deleted
  did not change the count (28 before, 30 after) on this small repository; it was reverted.
* linkding (DeepSeek, 66 features, 649 rules, see `issues/done/business_rules.md`): near-identical statements
  across features are 38 pairs, so duplicates are a smaller problem than on the small repository. The
  larger noise is user interface behaviour, the 70 rules of the features that describe end-to-end tests,
  and kinds that fall back on `computation` and `state_transition`; 450 of 649 rules are backed by code
  alone.

# Approach
1. Done once on linkding (figures above): measure by hand a sample for the share of noise and of true
   duplicates, and decide with it which of the points below is worth doing first (the features that
   describe tests, and the kinds, may matter more than deduplication).
2. Deduplication across features, candidates to compare: (a) mechanical, rules whose cited code lines
   overlap and whose statements share most words, kept once on the feature whose entry points own that
   code; (b) one extra LLM call per domain that merges the rules of its features; (c) ask each feature for
   the rules that its own entry points enforce, and give it the rules already stated by the features
   before it.
3. Noise: candidates (a) a stronger instruction with examples of what is not a rule; (b) a second
   judgement per rule ("would a business reader ask about this?") cheaper than the statement call;
   (c) lower the rank of rules backed by code alone in the rendered pages (they already show ⚠️).
4. Re-measure with the same figures and keep only what moves them.

# Resources
* `issues/done/business_rules.md` (smoke test section), ADR `0021`
* `crates/retrodoc-pipeline/src/rules/` (`prompt.rs`, `grounding.rs`, `mod.rs`)
* `.claude/skills/smoke-test/SKILL.md` (`just smoke <repo> --rules`)

# Hints
* A rule's slug is `<feature-slug>-<n>`: merging rules across features means choosing the feature whose
  page keeps it, and the rendering, MCP and report read that slug (`BusinessRule::belongs_to_feature`).
* Do not hard-code patterns of any language or framework to recognize plumbing: the same rule as the
  rest of the pass (the LLM judges, the checks are mechanical).

# Tracking

1. [x] Measure: run `--rules` on `delivery_router` and linkding (local Ollama), label a sample of 60 rules (fixed seed) as good, UI noise, test noise, plumbing, duplicate, wrong kind; count duplicate pairs; keep a reusable measure script and the reference figures here.
2. [x] Noise: a second judgement per rule ("would a business reader ask about this?"), also covering the features that describe tests; verdict logged, the noise dropped (no field on the rule); tested with a fake `LlmProvider` (filtering, incremental rerun, malformed answer keeps the rules); kept only if it lowers noise without losing good rules.
3. [x] (skipped, the noise left after deliverable 2 is small) Noise: rank rules backed by code alone lower in the rendered pages; only if deliverable 2 is not enough.
4. [x] Deduplication across features (2a only; (2c) dropped, see Decisions): a mechanical merge on overlapping cited lines and shared words, kept on the feature that owns the code (slug stays consistent with `belongs_to_feature`).
5. [x] Final re-measure with the figures of deliverable 1, before/after table, docs (`docs/crates/pipeline.md`, ADR if an architectural decision is taken).
6. [x] (5b, added after the first final measure) Features about tests: one question on the name and description, asked before the rules; such a feature has no rules. What is set aside (features, rules dropped as noise) is kept with its reason in `rules-set-aside.yaml` and listed in the debt report, so that the doubtful cases can be checked (asked by the user: a false positive must be visible).

## Decisions

* Deduplication: mechanical merge only for now; the rules a feature states are kept in `rules-stated.yaml`, `rules.yaml` is the merged set.
* Order: measure, then noise, then deduplication (linkding shows noise matters more than duplicates: 38 pairs among 649).
* Local Ollama (`qwen3.6:35b-a3b`, `reasoning_effort=none`) for every measure; the figures of the issue (DeepSeek) are not the baseline, the new ones are.
* The features that describe tests are handled by the LLM judgement, not by a hard-coded pattern; no language or framework patterns anywhere.
* The linkding run of 649 rules was not saved in the runs folder: deliverable 1 reruns it (a fresh clone of the public repository).

## Reference figures (deliverable 1, local `qwen3.6:35b-a3b`, 2026-10-16)

Measured with `cargo run -q -p retrodoc-pipeline --example rules_noise_measure -- <run dir>` (no LLM call;
duplicate pairs = different features, statements sharing at least 60% of their words, or 40% and
overlapping cited lines). Labelled by hand by the assistant, to be checked by the user.

| | `delivery_router` | linkding |
|---|---|---|
| rules / features with rules | 32 / 5 | 241 / 30 |
| backed by code alone | 31 | 133 |
| duplicate pairs (worded / same code) | 14 / 10 | 14 / 26 |
| rules in at least one pair | 14 | 40 (17%) |
| rules labelled (all / sample of 60, seed 1) | 32 | 60 |
| good | 9 (28%) | 32 (53%) |
| duplicate | 14 (44%) | 2 (3%) |
| plumbing | 9 (28%) | 8 (13%) |
| user interface noise | 0 | 7 (12%) |
| test noise (rules of "...-verification" features) | 0 | 11 (18%) |

* The measure finds 13 of the 14 duplicates labelled on `delivery_router` (it misses one that is worded
  differently). On linkding the sample shows only 2 duplicates, but 17% of the rules are in a pair.
* Small repository: the feature `system-initialization` restates almost every rule of the others
  (13 rules, 11 duplicates). Large one: the noise is the features that verify tests (14% of the rules
  are in a feature named `...-verification`, 18% of the sample), user interface behaviour and plumbing.
  This confirms the order of the plan: noise first.
* Wrong kind: about 4 of the 32 good rules of the sample (a filter or a display condition kind
  `state_transition` or `validation`, a condition kind `permission`); `computation` is still the fallback.
* Defect seen, not in this issue's scope: the local model truncated the answer (`finish_reason=length`,
  about 25k characters) for the 3 largest features (38 rules for `search-and-filtering`); two retried, one
  feature lost its rules (`user-profile-and-application-settings-management` was skipped), so
  the smoke test is not conclusive on linkding (rerun asked 2 calls). The same feature needs a smaller
  answer: to track in its own issue.

## Deliverable 2: judgement per rule (local `qwen3.6:35b-a3b`, 2026-10-16)

Replay of the judgement prompts outside the pipeline on the 241 saved rules of linkding, scored on the 60
hand labels of deliverable 1 (a throw-away script kept in the runs folder, one call per feature):

| variant | good rules dropped | noise dropped (26 labelled) |
|---|---|---|
| A: list the numbers to drop | 7 of 32 (22%) | 23 |
| B: one closed class per rule | 0 of 32 | 13 |
| C: B with sharper `test` and `technical` | 1 of 32 (3%) | 17 (65%) |

C is kept (`rules/judge.rs`). In the pipeline: linkding 229 statements, 67 dropped (29%), 162 kept (the 30 dropped
read by hand are almost all screen or test behaviour); `delivery_router` 33 statements, 4 dropped (3 plumbing,
1 formula whose two copies stay). Cost: one short extra call per feature with rules (74 calls in all for 28
features with rules, 212k tokens in and 54k out, 31 min; the judgement calls are the cheap ones).

* Estimated on the 60 labelled rules after C: good 31 of 46 kept (67%, from 53%), test noise 4, plumbing 4, screen 1,
  duplicates 2 (6 of the 46 are noise left: the judge stays prudent). The labels were not redone on the new
  pipeline run: the figures come from the replay.
* A small repository such as `delivery_router` has little to drop: its problem is the duplicates (deliverable 4).
* Not tried: the two classes left out of the judge (kind wrong, duplicates).
* Decision taken while doing it: the verdict is logged, not stored on `BusinessRule`; a verdict that can't be read keeps the rules as stated; the judgement prompt is part of the fingerprint.

## Deliverable 4: deduplication across features (local `qwen3.6:35b-a3b`, 2026-10-23)

Chosen: (2a), a mechanical merge (`rules/dedup.rs`), not (2c) (it makes the features depend on their order, ends
the parallel calls, and spoils the fingerprints). Read by hand, the 18 pairs of linkding and the 16 of
`delivery_router` before choosing the test:

| test on two rules of different features | false pairs | repeated rules found |
|---|---|---|
| 60% of the words alone | 3 of 14 pairs | most |
| 60% of the words and overlapping cited lines, or 75% of the words | none | about half |

Kept: the second one. Rules alike by 40 to 56% of the words with overlapping lines are a mix of true copies and
siblings (date added or date modified, favicons or web archive): no threshold tells them apart, so they stay.
Two traps handled and tested: "unknown customer" and "unknown restaurant" (67% of the words) are not joined
through a third rule (a group never holds two rules of one feature, and groups are joined only if all are alike);
the copy that stays is the one of the feature that owns the code.

| full run (stated, judged, merged) | rules stated | published | merged away | false merges (read by hand) |
|---|---|---|---|---|
| `delivery_router` | 29 | 23 | 6 | 0 |
| linkding | 226 | 218 | 8 | 0 |

* On `delivery_router` the merge removes most of what the judgement left: the time formula, the 60-minute limit,
  unknown customer and restaurant, the route. On linkding it removes only 4% (words of three letters or fewer are left out of the comparison, which costs one copy on each repository, 7 and 9 before that): 17% of the rules are in a pair
  but half are siblings that no mechanical test separates. If that is not enough, the next candidate is (2b)
  (one LLM call per domain); it was not tried.
* `rules-stated.yaml` (new artifact) keeps what each feature stated; a rerun starts from it, so that a copy comes
  back when its owner stops stating the rule (tested). A run saved before has only `rules.yaml`: it is read as
  the stated rules.
* The copy that stays takes the entities, use cases, tests, documents and the best confidence of the copies it replaces; a feature that reads no file never wins the tie. The rerun of both measured runs made no LLM call.

## Deliverable 5: final measure (2026-10-23)

Labelled by hand by the assistant, on the final `rules.yaml` of each run (same categories as deliverable 1).
`delivery_router` and linkding: local `qwen3.6:35b-a3b`; a large Rails application of the test set: DeepSeek
(`deepseek-chat`, `[llm.passes.rules]` only, the other passes read from the cache of a local run), 85 features,
about 4 minutes and 0.4 $ with the prices remembered for the model.

First measure (judgement per rule and merge, `c162158`):

| | `delivery_router` before / after | linkding before / after | large Rails application |
|---|---|---|---|
| rules | 32 / 23 | 241 / 218 | 1319 stated, 1179 published |
| sample labelled | 32 / 23 | 60 / 60 | 40 |
| good | 28% / 35% | 53% / 50% | 68% |
| duplicate | 44% / 39% | 3% / 2% | 0% (140 merged away, 10.6%) |
| plumbing | 28% / 26% | 13% / 18% | 28% |
| screen noise | 0 / 0 | 12% / 10% | 5% |
| test noise | 0 / 0 | 18% / 20% | 0% |

* Merges read by hand: 14 of the 140 drawn at random on the Rails application, all true copies; 0 false merge on
  the three repositories. The judgement dropped 38 of 1357 statements there (a backend with little screen
  behaviour).
* **The end-to-end sample of linkding did not improve** (good 53% to 50%). The cause is the test features: 9
  features named `...-verification` held 56 of 226 rules (25%), and the per-rule judgement left them
  (`new-bookmark-creation-verification`: 25 rules, all classed `business`, although its description says "Tests
  the new bookmark creation form"; a run earlier the same judgement had dropped 23 of those 25). The statements
  also vary a lot from a run to another (the same feature went from 8 to 32 rules).
* Replay outside the pipeline of a question asked per feature, on its name and description only ("does it
  describe how the application is tested?"): right for the 30 features of linkding (9 yes, all the
  `...-verification` ones, 21 no).

Second measure, with that question asked before the rules (deliverable 5b; a feature about tests gets no rule and
no statement call; the sentence about tests left the per-rule judgement):

| | linkding | large Rails application |
|---|---|---|
| features about tests | 14 of 33, all named `...-verification` | 0 of 85 |
| rules | 112 | 1349 stated, 1197 published |
| sample labelled | 60 | not redone (no feature about tests, so nothing changes in it) |
| good | 63% | 68% (first measure) |
| duplicate | 2% | 0% |
| plumbing | 22% | 28% |
| screen noise | 13% | 5% |
| test noise | **0%** (from 18% and 20%) | 0% |
| LLM calls | 98 (35 min) against 77 | 256 against 170 (the question: 85 short calls) |

* On linkding the 14 features about tests held 100 of the 218 rules of the first measure; they are gone. The
  rest of the noise is plumbing (22%) and screen behaviour (13%), which the per-rule judgement leaves in part.
* The good rules of the labelled sample are not a stable count: 128 (53% of 241) at the start, 109, then 71
  (63% of 112): the statement pass gives different rules from a run to another, and the features about tests
  held some rules that read as good (a tag name must be unique) which other features state too. Not measured
  on a fixed set of statements.
* Not solved here: the plumbing left (about a fifth), the siblings the merge does not join (17% of the rules of
  linkding are in a pair of the loose test, half of them siblings), and the variance of the statement pass.
* Trace of the doubtful cases (asked by the user): the 14 features of linkding taken for tests and the 34
  rules dropped as noise are listed in the debt report with their class; reading them, a few drops are
  debatable (a rule on the re-indexing of the other bundles when one is deleted, the one on creating a tag with
  its owner), which is the kind of case the list is for. No way yet to put a feature back by hand: to do if a
  wrong yes shows up.
