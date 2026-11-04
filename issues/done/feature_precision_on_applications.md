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
2. [x] Replay baseline: `pass_replay features` x3 on the dafea18 frozen inputs of linkding and Lobsters (no features
   replay exists on them), hand precision by the classes of step 1 on one run each; verified by a ledger line. (autonomous; nothing to commit but this tracking)
3. [x] Technical features set aside: `technical` in the features answer, set aside from `features.yaml`, the
   placement and the unplaced count, listed in the coverage and the debt report; verified by tests through the pass
   and the report with a fake LLM; makes true TEF-1 to TEF-7. (autonomous)
4. [x] Replays of candidate T: x3 on both applications against step 2 (judged and hand precision, recall, set aside
   features read by hand for real features lost); verified by a ledger line. (autonomous)
5. [x] Candidate M, only if the too fine ones stay above a fifth of the unmatched after step 4: prompt rule that the
   notifications, settings pages and API of a capability go in its feature; replays x3 both applications.
   (autonomous; measured, not kept)
5b. [x] Candidate T3: the operations commands, scripts and scheduled jobs (deployment, maintenance, backup) are
   technical, a back-office tool is what an administrator uses in the application; replays x3 both applications;
   kept. (autonomous)
6. [x] Whole benchmark of the pick (hidden series, 3 runs, both applications), record in `docs/findings.md`, "Latest figures" of
   `PRODUCT.md`, the ledger. (autonomous)
7. [x] Docs and fixes of the `high` review. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: d40306c2718be7d6242398ff5026db656c6e574d
- Technical features: left out of the docs and the use cases, listed in the debt report (question 1 a).
- Whole benchmark: hidden series, 3 runs, both applications, about $1.8 (question 2 a).
- Put-back key for a technical feature: a follow-up issue, not this one (question 3 a).
- Paid replays of deliverables 2, 4 and 5 agreed, about $0.12 a series of 3 on both applications (question 4 a).
- After deliverable 5: one more replay T3 (operations commands and jobs technical, about $0.1), then the whole
  benchmark of the better of T2 and T3 (agreed 2026-11-03).

## Deliverable 1: classification
- done: `docs/findings.md` §10, the Findings of this issue, the ledger; the spec draft `technical-features-set-aside`
  and the follow-up `issues/done/technical_feature_put_back.md` (in the milestone) go with it.

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | 1 finding, settled |

red: none (measure and docs)

Code review: 1 findings
- the plan line not ticked: ticked at the tracking step, as the workflow orders it.

## Deliverable 2: replay baseline
- done: `pass_replay features` x3 at fbdbf54 on both frozen inputs (`feature_precision_on_applications-fbdbf54/base-*`),
  about $0.1.

| | linkding | Lobsters |
|---|---|---|
| features | 45 (42-47) | 91 (90-92) |
| recall, judge | 90% (88-96) | 98% (94-100) |
| precision, judge | 50% (49-51) | 42% (41-43) |
| run 1 unpaired, technical by hand | 12 of 22 | 31 of 62 |

- not verified: the full hand precision of the replays (only the technical ones of run 1 counted); the Lobsters
  judge leaves right pairs out on these unaudited runs (comments, sign in, flags among the 62), so compare judged
  with judged.

## Deliverable 3: technical features set aside
- done: `technical` in the features answer (prompt and schema), `Feature.technical`; technical features saved in
  `features-set-aside.yaml` (cleared by `--force`), never in `features.yaml`, read back as the previous run; held
  entry points, never a placement target; debt report section "Technical features set aside"; `pass_replay` keeps
  `features-set-aside-<n>.yaml`; spec `technical-features-set-aside` accepted; the pinned print of a domain updated
  (the prompt changed on purpose: every domain is asked again once).

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | no findings |

red: TEF-1, TEF-2, TEF-3, TEF-4, TEF-5, TEF-6: `features.yaml` held `Test data generation` (assertion at
`technical_features.rs:74`); TEF-7 written with the code, green at its first run.

Code review: no findings

## Deliverable 4: replays of candidate T
- done: replays x3 on both applications of T (38c3457) and of T2, its prompt refined after reading what T set aside
  (an exchange with an external system or a user, or a copy of what a user saved, is not technical; committed here,
  the pinned print updated again).

| | baseline | T | T2 |
|---|---|---|---|
| linkding features | 45 (42-47) | 39 (36-43) | 39 (39-40) |
| linkding recall / precision, judge | 90% / 50% (49-51) | 94% / 55% (51-61) | 93% / 53% (51-55) |
| linkding set aside per run | - | 6-8 | 5-7 |
| Lobsters features | 91 (90-92) | 60 (59-62) | 65 (64-67) |
| Lobsters recall / precision, judge | 98% / 42% (41-43) | 88% / 54% (50-58) | 91% (87-97) / 47% (44-51) |
| Lobsters set aside per run | - | 20-27 | 15-17 |

- T set aside real features: `Bookmark Metadata and Snapshots` (14 entry points, the page archiving of the reference)
  on 2 linkding runs, `Webmentions` and `Mastodon posting` (the Syndication domain) on 2 Lobsters runs. T2 sets aside
  none of them; what it sets aside is test data, caches, keystore, short ids, asset builds, dev checks, parsing helpers.
- Lobsters T2 run 1 lost 12 entry points of the comments (no feature of its domain answer held them, the placement
  left them out; not set aside): its 87% recall; runs 2 and 3 hold them all. Variance of the model, to watch at the
  whole benchmark.
- Too fine ones still about a third of the unpaired of Lobsters T2 run 1 (hats, avatars, linked accounts, two-factor,
  notifications of messages, mod mail threads): over a fifth, so candidate M goes on.
- not verified: hand precision of these replays; the judge's pairs on them are unaudited.

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | no findings |

red: none (a prompt refinement measured by the replays; the pinned print test covers the request)

Code review: no findings

## Deliverable 5: candidate M
- done: replays x3 on both applications of T2 plus the rule "keep a capability whole: its notifications, settings, API
  endpoints and variants go in its feature" (`feature_precision_on_applications-m/`, about $0.1); not kept, the rule
  reverted, nothing committed but this tracking.

| | T2 | M |
|---|---|---|
| linkding features | 39 (39-40) | 36 (36-37) |
| linkding recall / precision, judge | 93% / 53% (51-55) | 97% / 54% (51-58) |
| Lobsters features | 65 (64-67) | 68 (66-70) |
| Lobsters recall / precision, judge | 91% / 47% (44-51) | 96% / 51% (48-58) |
| Lobsters set aside per run | 15-17 | 12-19 |

- Why not kept: inside the ranges of T2; the too fine ones it aimed at stay (linkding: favicons, web archive
  fallback, token, profile API, custom CSS; Lobsters: avatars, linked accounts, hats, message notifications), and it
  sets aside fewer technical ones. Lobsters run 3 again lost 14 entry points held by no feature (as T2 run 1).
- Left: on linkding, backups, health check, deployment commands and admin provisioning stay features under T2: the
  rule "a back-office tool is not technical" keeps the operators' commands in.

## Deliverable 5b: candidate T3
- done: the back-office exception of the technical rule narrowed to the screens and actions an administrator uses
  in the application, the commands, scripts and scheduled jobs of deployment, maintenance or backup made technical;
  replays x3 (`feature_precision_on_applications-t3/`, about $0.1); kept, the pinned print updated.

| | T2 | T3 |
|---|---|---|
| linkding features | 39 (39-40) | 36 (35-37) |
| linkding recall / precision, judge | 93% / 53% (51-55) | 89% (88-92) / 58% (57-60) |
| linkding set aside per run | 5-7 | 10 |
| Lobsters features | 65 (64-67) | 69 (54-81) |
| Lobsters recall / precision, judge | 91% (87-97) / 47% (44-51) | 95% (87-100) / 55% (53-57) |
| Lobsters set aside per run | 15-17 | 19-22 |
| entry points held by no feature, Lobsters | 31-43 | 31 every run |

- Set aside now on linkding: backups, admin provisioning at deployment, database maintenance, secret key, health
  check, besides the helpers; read by hand, none holds a reference feature (`Snapshot processing jobs` of run 2 is the
  task workers, the snapshot routes stay in their feature).
- not verified: hand precision; the linkding judged recall dips 4 points, its run 1 misses `Use the REST API` that
  T2 found, inside the noise of an unaudited judge.

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | no findings |

red: none (a prompt refinement measured by the replays; the pinned print test covers the request)

Code review: no findings

## Deliverable 6: whole benchmark of T3
- done: `just benchmark --provider deepseek --runs 3 --series hidden` at f95ada6 on both applications (no `FAIL`, no
  warning, about $1.6); hand audit of `hidden-1` of each, the right pairs (the judge's and 8 it missed) added to
  `matches.yaml`, 1 wrong one rejected, pairs equal once normalised left out; these runs and the dafea18 hidden runs
  re-scored `--saved-judge` with the same pairs; `docs/findings.md` §10, "Latest figures" of `PRODUCT.md`, ledger.

| figure | linkding dafea18 | linkding f95ada6 | Lobsters dafea18 | Lobsters f95ada6 |
|---|---|---|---|---|
| features recall, judged | 90% | 89% (83-92) | 96% | 98% (97-100) |
| features precision, judged | 57% | 63% (59-66) | 52% | 65% (56-77) |
| features precision by hand, `hidden-1` | 49% | 68% | 45% | 78% |
| domains precision, judged | 100% | 75% (62-88) | 85% | 64% (50-78) |

- not verified: why the domains precision fell: the domains pass is unchanged since dafea18 and runs before the
  features; linkding now gives 8 domains each run (6 at dafea18), one with no feature.
- follow-up: `issues/done/domain_precision_after_technical_set_aside.md` (filed with the fixes of the `high` review).

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | no findings |

red: none (measure and docs)

Code review: no findings

## Deliverable 7: docs and fixes of the `high` review
- done: `docs/crates/pipeline.md` says the final rule; a feature merged from a later slice is technical only when both
  answers are; the known features of the earlier slices carry `(technical)`; `features-set-aside.yaml` written
  before `features.yaml`; no clone to split the features; the print of a domain updated (the mark is part of the
  request; no domain of the measured runs was sliced, so the f95ada6 figures stand); follow-up issue on the domains.
- not fixed: the files of a set-aside feature listed again under "Technical support" (they are absent from the
  functional docs, which that list says); the table cells of the new section not escaped and the domain shown by its
  slug (as the other tables of the report); the technical filter repeated in the placement helpers (the slice stays
  mutable for the placement, the helpers take `&[Feature]`).

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | no findings |

red: none: `a_feature_merged_from_a_later_slice_is_technical_only_when_both_are` written with the fix (no spec id:
a property of the slices, not a requirement of TEF)

Code review: 8 findings (the `high` review of the issue)
- 1, 2: slices merge and known features (fixed); 4: order of the two saves (fixed); 8: clones (fixed).
- 3: empty domain page, outside TEF: `issues/done/domain_precision_after_technical_set_aside.md`.
- 5, 6, 7: not fixed, reasons above.
