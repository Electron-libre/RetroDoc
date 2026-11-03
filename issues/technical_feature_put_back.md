---
priority: low
depends_on: [feature_precision_on_applications]
why: 'a comfort: the debt report already lists the technical features set aside, a wrong one is seen there'
---
# Put back a feature the features pass set aside as technical

# Goal
`feature_precision_on_applications` sets aside, on a judgement of the LLM, the features it marks technical
(development, tests, deployment, internal plumbing): they leave the functional docs and the debt report lists
them. A user who finds there a real feature set aside has no way to put it back. Expected: a key of `retrodoc.toml`
that puts a technical feature back, held across reruns and `generate --force`, as `keep_features` of `[rules]` does
for the features the rules pass takes for tests.

# Findings
* Hand classification of 2026-11-03 (`issues/feature_precision_on_applications.md`): of the unmatched generated
  features, 32 of 51 on Lobsters and 12 of 22 on linkding are technical; some, such as an inbound e-mail route or a
  mailing list job, sit close to a user feature, so a wrong mark is to be expected.
* The rules pass already has this pattern: `[rules] keep_features`, `keep_rules` (`specs/rules-set-aside.md`).

# Approach
1. Read how often the set-aside list of the whole benchmark of `feature_precision_on_applications` holds a real
   feature; if never, close this issue as not needed.
2. Candidates: (a) `keep_features` in a `[features]` section, by name and domain since a set-aside feature has no
   slug in `features.yaml`; (b) by entry point id, the entry points of the listed id placed again.

# Specs
* `specs/technical-features-set-aside.md`: new requirements for the put-back (a non-goal of the spec today).

# Milestone
Measured quality at the current code

# Resources
* `specs/rules-set-aside.md`, `crates/retrodoc-core/src/config.rs` (`RulesConfig`).
* `crates/retrodoc-pipeline/src/features/from_surface.rs`, `crates/retrodoc-pipeline/src/behaviour_coverage.rs`.

# Hints
* The features fingerprint must take the list, so listing a feature redoes its domain only (as RSA-2).
