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
