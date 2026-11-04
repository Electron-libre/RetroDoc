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
* Hand classification of 2026-11-03 (`issues/done/feature_precision_on_applications.md`): of the unmatched generated
  features, 32 of 51 on Lobsters and 12 of 22 on linkding are technical; some, such as an inbound e-mail route or a
  mailing list job, sit close to a user feature, so a wrong mark is to be expected.
* Set-aside lists of the whole benchmark at f95ada6 read by hand (2026-11-04): 83 features over the 6 hidden runs
  (linkding 10, 11, 11; Lobsters 11, 29, 11), none a reference feature the docs lose; the closest (linkding HTML
  snapshot worker, hat and invitation routes, story card job, import helper) are parts of a feature the same run
  keeps. Not needed on the benchmark repositories.
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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
Step 1 of the Approach is read (by hand, no LLM): the 83 features set aside by the 6 runs of the whole benchmark at
f95ada6 (linkding 10, 11, 11; Lobsters 11, 29, 11) hold no reference feature the docs lose: helpers, route files,
operations scripts and jobs, test data, or a part of a feature that its run keeps (hat and invitation routes, story
card job, import helper). So the issue closes as not needed, no key added; out of scope: the prompt of the mark.

## Plan
1. [x] Record the reading and close: the Findings of this issue, `docs/findings.md`, the non-goal of
   `specs/technical-features-set-aside.md` (the put-back not needed, measured), a ledger line; verified by
   `cardinal run project-check` and a reread; then `cardinal run close-issue`. (autonomous)

2. [x] Fixes of the `high` review: the claims narrowed to what was read (reference features, hidden runs, the
   benchmark repositories), the snapshot worker named, the finding dated. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 46cf50cd63888a751f86385d641bc69f5338ba22
- Close as not needed, the non-goal of the spec points at the measure (question 1 a).

## Deliverable 1: reading and close
- done: the set-aside lists of the 6 hidden runs at f95ada6 read against the references; recorded in the Findings,
  `docs/findings.md` §10, the non-goal of `specs/technical-features-set-aside.md`, the ledger; no key added.

| check | result |
|---|---|
| tests, lint, specs (`cardinal run project-check`) | pass |
| code review (`low`) | 1 finding, fixed |

red: none (measure and docs)

Code review: 1 findings
- `docs/findings.md` cited the `issues/done/` path before the close: now the open path, which `close-issue` rewrites.

## Deliverable 2: fixes of the `high` review
- done: `docs/findings.md` and the spec non-goal say "reference feature", "6 hidden runs", "on the benchmark
  repositories"; the linkding HTML snapshot worker named among the closest; the operator tasks absent from the
  references said not judged; the finding dated 2026-11-04.

| check | result |
|---|---|
| tests, lint, specs (`cardinal run project-check`) | pass |
| code review (`low`) | no findings |

red: none (docs)

Code review: 5 findings (the `high` review of the issue)
- 1, 3, 5: claims narrowed (fixed); 2: snapshot worker named (fixed); 4: date added (fixed).
