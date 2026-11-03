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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
The baseline audit at dafea18 already answers step 1 and 2 of the Approach: the judge errs both ways within 4
points, not 12 to 33, so no change of the judge (candidates a, b, c: b is already BSC-2). What is left is
candidate (d): make the hand audit count and say, per run, how much of the judged figure no hand has checked.

## Plan
1. [x] Rejected pairs and unaudited count: `rejected` lists in `matches.yaml` (the wrong pairs now in its comments
   moved there, both applications), judge proposals equal to a rejected pair dropped from the judged figures, the
   unaudited proposals counted in the run report, the summary and `benchmark-table`; `SCORING_VERSION` 3;
   verified by tests through the scoring and the table; makes true BSC-16, BSC-17, BSC-18. (autonomous)
2. [x] Re-score and record: the 12 runs of the baseline re-scored `--saved-judge` (no LLM call), judged against
   hand precision and the unaudited counts in `docs/findings.md`, the ledger and the skill `quality-benchmark`
   (audit step writes the wrong pairs to `rejected`); verified by the re-scored tables and `just check`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 02d7c386be24b73e2b4ea3f2ae420847c8eeff5e
- Scope: candidate (d) only, the judge unchanged (no paid re-judge).

## Deliverable 1: rejected pairs and unaudited count
- done: `rejected` lists in `matches.yaml` (10 domain and 16 feature pairs on linkding, 1 and 4 on Lobsters, moved
  from the comments; the pairs too generic to key on stay as comments), dropped from the judged figures; the
  unaudited proposals counted in the run report, the summary and `benchmark-table`; `SCORING_VERSION` 3; spec,
  crate docs and the audit step of `quality-benchmark` updated (planned in deliverable 2, it belongs to this change).
- Re-score of linkding `hidden-1` (dafea18, `--saved-judge`): features judged 49% (hand 49%), 0 feature pair
  unaudited, 2 domain pairs unaudited (the catch-all `Bookmark Management`).

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| size | 578 lines, within |
| code review (`low`) | no findings |

red: BSC-16, BSC-18: the rejected `Bundle services` still counted as matched (`left: []`, `right: ["Bundle
services"]`); BSC-17: a rejected pair naming `Typo` scored without error.

Code review: no findings

## Deliverable 2: re-score and record
- done: the 12 runs of the dafea18 baseline re-scored `--saved-judge` at dd5cf43
  (`judge_overstates_feature_precision-dd5cf43-<repo>/`, 0 FAIL); figures in `docs/findings.md` section 9, the
  "Latest figures" of `PRODUCT.md` and the ledger (measure line, G7 ticked). The skill was updated in deliverable 1.
- Audited run-1s: judged feature precision = hand (49%, 67%, 45%, 46%), feature pairs unaudited 0, 0, 3, 1. Runs
  2-3, never audited: 5 to 28 feature pairs unaudited each.

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | 1 finding, deferred |

red: none (measure and docs)

Code review: 1 findings
- `matching.rs`: `check_references` repeats the normalized lookup of the `reference_index` closure of `score`;
  code of deliverable 1, left to the fixes of the `high` review of the issue.

## Deliverable 3: docs and fixes of the `high` review
- done: `PRODUCT.md` risk line (12 to 33 points was the earlier audits); BSC-19 (a pair both kept and rejected is an
  error); one lookup of a reference name in `matching.rs`; `Unaudited` carried in the judged tuple of `RunReport::new`;
  summary heading "less the ones matches.yaml rejects"; the plural `RSS/Atom Feeds` rejected pairs dropped (equal
  once normalized); redundant serde attributes dropped; findings say what the 3 and 1 unaudited pairs are.
- not fixed: a share of unaudited pairs instead of a count (the count reads against the number of features of the
  run, beside it in the table; BSC-18 states a count).

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | no findings |

red: BSC-19: a pair both in `features` and `rejected.features` scored without error.

Code review: no findings

All deliverables done; nothing left. Final measure: on the audited runs the judged feature precision equals the
hand one; the runs never audited say how many of their pairs rest on the judge alone.
