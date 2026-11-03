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
* Not measured: whether the unmatched features are wrong, too fine (a reference feature cut in several), technical,
  or right but missing from the reference.

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
