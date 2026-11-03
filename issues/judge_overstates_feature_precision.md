---
priority: high
depends_on: [benchmark_baseline_at_head]
why: 'feature precision is the weakest figure and the one the judge overstates most: it cannot be improved while it cannot be read'
---
# Make the judged feature precision believable

# Goal
The benchmark judge pairs generated features with reference features; on the audited runs it was wrong on a large
share of its pairs and overstated the feature precision by 12 to 33 points. A change that improves the features can
then read as noise, and one that harms them as a gain. Expected: a judged feature precision within a few points of a
hand audit on both benchmark applications, or a measure that says how far to trust it.

# Findings
* `issues/done/benchmark_passes_on_fixed_inputs.md` (audit of the judge): 10 of 23 feature pairs wrong on
  `delivery_router`, 3 of 22 on linkding; recall overstated by 8 to 17 points, precision by 12 to 33.
* Lobsters 2026-10-27 (`second_application_in_benchmark`): judge pairs wrong 2 of 49.
* linkding 2026-10-28 (`brief_injection_experiments`): "judge unreliable (no pair proposed in 2 runs)"; the empty
  answers were fixed by `issues/done/judge_silently_proposes_no_feature_pair.md` (names read before descriptions,
  retry, unjudged runs left out of the mean).
* A generated feature now counts for each reference feature it holds
  (`issues/done/one_generated_name_for_several_reference_items.md`): a broad feature raises recall without lowering
  precision.
* Baseline at dafea18 (`issues/done/benchmark_baseline_at_head.md`, 2026-11-03), one run per series audited by hand:
  feature pairs wrong 3/20, 1/25 (linkding), 0/33, 1/33 (Lobsters); right pairs missed (not already hand pairs)
  0, 1, 2, 3; hand feature precision 49%, 67%, 45%, 46% against 53%, 67%, 42%, 44% judged at the run: within 4
  points, both ways, not 12 to 33. On Lobsters the judge understates the domain precision (hand 89%, 88%; judged 78%,
  75%). The checked pairs are in `matches.yaml`.

# Approach
1. Measure first on the runs of `benchmark_baseline_at_head`: label every judged feature pair by hand (right, wrong,
   partial) on one run per application, and compute the hand precision next to the judged and strict ones.
2. Sort the wrong pairs by cause (a broad generated feature paired with many references, near names, a technical
   feature paired with a business one, ...).
3. Candidates by cause (can be combined): (a) the judge sees the use cases or the entry points of each generated
   feature, not only its name and description; (b) a one-to-many pair counts once in precision; (c) a second judge
   call that checks each proposed pair; (d) report a "pairs audited" share and keep hand-audited samples as the
   reference of the judge (a `confidence_agreement`-like replay).

# Milestone
Measured quality at the current code

# Resources
* `crates/retrodoc-pipeline/src/benchmark/judge.rs`, `matching.rs`, `metrics.rs`, `replay_judgement.rs`.
* Skill `quality-benchmark`, `specs/benchmark-scoring.md`.
* Closed issues named in the findings.

# Hints
* `--saved-judge` re-scores without an LLM call: use it to compare scoring changes on the same answers.
* A judge change makes the earlier judged figures incomparable: re-judge the baseline runs in the same deliverable.
