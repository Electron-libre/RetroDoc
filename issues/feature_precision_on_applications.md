---
priority: high
depends_on: [benchmark_baseline_at_head, judge_overstates_feature_precision]
why: 'about half the generated features of an application match no reference feature: the weakest figure of the docs a reader sees first'
---
# Raise the feature precision on applications

# Goal
On Lobsters about half of the generated features match no reference feature (judged precision 51%, and the judge
overstates it); on linkding the hand precision is about 77%. A reader taking an application over meets these
features first, under each domain. Expected: the causes of the unmatched features known, and a change of the features
pass that raises the hand precision on both applications without losing recall, measured on frozen inputs.

# Findings
* Lobsters, `surface` path, 2026-10-28 (7e2bf9a): 73 features, judged recall 100%, precision 51%; `files` path 98
  features, 97% / 47%.
* Lobsters features replays on frozen inputs (2026-10-29, a07df3c): 79-80 features, judged precision 44-47%; the
  routing filter had no effect.
* linkding features replays (2026-10-28, d600304): 28-36 features, precision by hand 73-79% by setting.
* A 9-file library: the entry points pass listed constructors and internal collections as features
  (`docs/findings.md` §7); fixed for applications by leaving out the public methods no route reaches.
* Unmatched features of the audited `hidden-1` runs of dafea18, by hand (2026-11-03, `docs/findings.md` §10):
  Lobsters 51 = 32 technical, 10 too fine, 7 right or missing from the reference, 1 back-office, 1 wrong; linkding
  22 = 12 technical, 9 too fine, 1 missing from the reference. The features with no HTTP route are almost all
  unmatched (33 of 37, 10 of 13).

# Approach
1. Measure first: classify the unmatched generated features of one Lobsters and one linkding baseline run by hand:
   too fine, technical or back-office, duplicate, wrong, missing from the reference (then fixed in the reference by
   `benchmark_references_checked_by_a_human`).
2. Candidates by the dominant cause, each replayed 3 times on frozen inputs (cents on linkding, about $0.1 on
   Lobsters): (a) merge pass or prompt rule for features that share entry points or entities; (b) a minimum of
   entry points per feature, the rest folded into a neighbour; (c) back-office features left out of precision when
   the reference sets them apart; (d) the brief or the domain description given as the frame of the features.
3. The whole-run benchmark only for the candidate the replays pick.

# Milestone
Measured quality at the current code

# Resources
* `crates/retrodoc-pipeline/src/features/` (`from_surface.rs`), `specs/` of the features pass if any.
* `crates/retrodoc-bench/examples/pass_replay.rs`, the frozen inputs (outside git, skill `quality-benchmark`).
* `issues/done/brief_injection_experiments.md`, `issues/done/surface_domains_default_decision.md`.

# Hints
* Paid replays: ask the user first, with the estimated cost.
* Recall counts a generated feature for each reference feature it holds: merging features can keep recall while it
  raises precision, check the merged ones stay readable.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
Step 1 of the Approach is measured (by hand, no LLM, `~/retrodoc-runs/feature_precision_on_applications-audit.md`):
of the unmatched features of the audited `hidden-1` runs, technical ones dominate (Lobsters 32 of 51, linkding 12 of
22: test data, dev checks, caches, helpers, health), then too fine ones (10, 9), right or missing from the reference
(7, 1), wrong (1, 0). Means: a `technical` mark in the features answer, the marked features set aside out of the docs
(spec `technical-features-set-aside`), then a merge rule if the too fine ones still weigh; out of scope: the judge,
the references (`benchmark_references_checked_by_a_human`).

## Plan
1. [x] Record the classification: dated entry in `docs/findings.md`, the Findings of this issue, the ledger; verified
   by `just check` and a reread. (autonomous)
2. [ ] Replay baseline: `pass_replay features` x3 on the dafea18 frozen inputs of linkding and Lobsters (no features
   replay exists on them), hand precision by the classes of step 1 on one run each; verified by a ledger line.
3. [ ] Technical features set aside: `technical` in the features answer, set aside from `features.yaml`, the
   placement and the unplaced count, listed in the coverage and the debt report; verified by tests through the pass
   and the report with a fake LLM; makes true TEF-1 to TEF-7.
4. [ ] Replays of candidate T: x3 on both applications against step 2 (judged and hand precision, recall, set aside
   features read by hand for real features lost); verified by a ledger line.
5. [ ] Candidate M, only if the too fine ones stay above a fifth of the unmatched after step 4: prompt rule that the
   notifications, settings pages and API of a capability go in its feature; replays x3 both applications.
6. [ ] Whole benchmark of the pick (hidden series, 3 runs, both applications), record in `docs/findings.md`, "Latest figures" of
   `PRODUCT.md`, the ledger.

## Decisions
- Mode: autonomous
- Base commit: d40306c2718be7d6242398ff5026db656c6e574d
- Technical features: left out of the docs and the use cases, listed in the debt report (question 1 a).
- Whole benchmark: hidden series, 3 runs, both applications, about $1.8 (question 2 a).
- Put-back key for a technical feature: a follow-up issue, not this one (question 3 a).
- Paid replays of deliverables 2, 4 and 5 agreed, about $0.12 a series of 3 on both applications (question 4 a).

## Deliverable 1: classification
- done: `docs/findings.md` §10, the Findings of this issue, the ledger; the spec draft `technical-features-set-aside`
  and the follow-up `issues/technical_feature_put_back.md` (in the milestone) go with it.

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | 1 finding, settled |

red: none (measure and docs)

Code review: 1 findings
- the plan line not ticked: ticked at the tracking step, as the workflow orders it.
