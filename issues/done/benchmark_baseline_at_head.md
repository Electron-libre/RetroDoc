---
priority: high
why: 'the measured figures predate several changes of the passes; every quality issue of the milestone compares with this baseline'
---
# Measure a benchmark baseline at the current code

# Goal
The last whole benchmark runs date from 2026-10-28; since then the docs language, the narrative check in the
confidence call, the new rules judgement, the reconciliation by identifiers and many fixes landed. Nobody knows the
figures of the code as it is. Expected: one fresh baseline per benchmark application at the current commit, with its
spread, recorded in the ledger and in `docs/findings.md`, that the other issues of the milestone compare with.

# Findings
* Lobsters (2026-10-28, `deepseek-chat`, 1 run, confidence off): judged domains 100% recall / 82% precision, features
  100% / 51%, narratives in business language 81%, 126 calls, 9 min, about $0.3.
* linkding (2026-10-28, features replays on frozen domains, 3 runs): features found by hand about 75%, precision by
  hand about 77%; use cases replay (1421894): business 78%, score=1 75%.
* The G2 baseline of the domains pass on linkding is not saved (figures of 2026-10-23 lost).
* `delivery_router` saturates at 100%: a smoke repository, not a benchmark one.
* Since 0816a1c/7c64fe6 the system prompts are part of the fingerprints: a `generate` on a run saved before is a
  full run, not a rerun.

# Approach
1. Read the ledger: reuse any measure still `valid` at the current commit (`git diff <commit>..HEAD` on the pass).
2. With the user's go (estimate about $1: Lobsters about $0.3 a run, linkding less), one `just benchmark` series
   on each of linkding and Lobsters, hidden then shown, 2 to 3 runs, confidence on (the narrative check is new).
3. Re-freeze the inputs of both applications at that commit (`freeze_inputs`) if the surface passes changed, and
   replay the domains pass 3 times on linkding to restore its missing baseline (cents).
4. Write the figures with their spread in the ledger and a dated entry in `docs/findings.md`, and update the
   "Latest figures" paragraph of `PRODUCT.md`.

# Milestone
Measured quality at the current code

# Resources
* Skill `quality-benchmark`, the ledger of measures (outside git, see `CLAUDE.md`), `benchmark/linkding/`, `benchmark/lobsters/`.
* `issues/done/second_application_in_benchmark.md`, `issues/done/surface_domains_default_decision.md`.

# Hints
* Paid run: ask the user first, with the estimated cost.
* The judged precision is overstated (see `judge_overstates_feature_precision`): report the strict and judged figures
  side by side, and audit a sample of feature pairs by hand.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
One fresh whole-run baseline per benchmark application (linkding, Lobsters) at the current commit, hidden and
shown, with its spread, confidence on; re-frozen inputs and the lost linkding domains replay baseline. No code
change: the committed output is the audited pairs and the figures in `docs/findings.md` and `PRODUCT.md`.

## Plan
1. [x] Benchmark series: `just benchmark --provider deepseek --runs 3` on linkding then Lobsters, both series,
   confidence on; verified by every run present in `table.md` (no `FAIL`) and a ledger line per application. (autonomous; no file of the repo, nothing to commit)
2. [x] Re-freeze: `freeze_inputs` of both applications from a `hidden-1` clone of step 1 (the surface passes
   changed since 7e2bf9a: 1,350 lines in `roles`, `entry_points`, `business_*`, `domains`, `brief`, `actors`),
   then `pass_replay domains` x3 on linkding; verified by the manifests and a ledger line. (autonomous; nothing to commit)
3. [x] Audit: hand check of the judge's domain and feature pairs of one run per series and application, strict and
   judged precision side by side; checked pairs added to `benchmark/<repo>/matches.yaml`; verified by
   `retrodoc benchmark --saved-judge` re-scores and `just check`. (autonomous)
4. [x] Record: dated entry in `docs/findings.md`, "Latest figures" of `PRODUCT.md`, ledger (measures, reusable
   artifacts, G7 ticked); verified by `just check` and a reread. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: dafea18e80509629ec8675291a69e137e8129921
- Runs: 3 per series, both applications, both series (about $4); re-freeze and domains replay x3 on linkding (cents).

## Deliverable 1: benchmark series
12 runs of 12, no `FAIL`; warnings of known kinds only (refused JSON schema, empty batched parts, `story.rb` cut, 6
entry points no feature holds). Tables: `retrodoc-benchmark-linkding/1793704964/table.md`,
`retrodoc-benchmark-lobsters/1793704970/table.md`; cost about $3.5, from the tokens.

## Deliverable 2: re-freeze
Both applications re-frozen from their `hidden-1` clone (copied to `benchmark_baseline_at_head-dafea18-<repo>-1`, the
new `source_run`); the former frozen inputs kept as `frozen/<repo>-before-dafea18`. Linkding domains replay x3: 6
(5-7) domains, judged 100%/100% each run.

## Deliverable 3: audit
- done: judge pairs of hidden-1 and shown-1 of each application checked by hand; right pairs and the right ones the
  judge missed added to `matches.yaml`, wrong ones listed in its header; 12 runs re-scored `--saved-judge`.
- Judge, on the 4 audited runs (linkding hidden, shown, Lobsters hidden, shown): feature pairs wrong 3/20, 1/25, 0/33,
  1/33 (+1 doubtful); right pairs missed, not already hand pairs: 0, 1, 2, 3 (+1 domain on each Lobsters run). Hand vs
  judged-at-run feature precision: 49% / 53%, 67% / 67%, 45% / 42%, 46% / 44%.

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| size | 283 lines, within |
| code review (`low`) | no findings |

red: none (measure, no test)

Code review: no findings

## Deliverable 4: record
- done: section 9 of `docs/findings.md`, "Latest figures" of `PRODUCT.md`, ledger (3 measures, reusable artifacts,
  G2 domains and G7 ticked); the audit's figures added to the findings of `judge_overstates_feature_precision`.
- follow-up: that issue assumed an overstatement of 12 to 33 points; this audit finds within 4, both ways.

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | 1 finding, fixed |

red: none (docs)

Code review: 1 findings
- `PRODUCT.md`: the narratives range mixed the shown series into figures said hidden; now 82% on both (fixed).

## Deliverable 5: fixes of the `high` review
- done: the judge's misses recounted without the pairs already in `matches.yaml` (findings, both issues); the Lobsters
  domain precision given with the audited pairs (85%, 88%) beside the 72% at the run; redundant pairs (equal once
  normalised) and names too generic to key on dropped from `matches.yaml`, the generic ones listed in its header; the
  stale header line on bundles fixed; ranges for recall and precision in the findings table; `ROADMAP.md` Why line.
- 12 runs re-scored again (`-rescored/`).

| check | result |
|---|---|
| tests, lint, specs (`just check`) | pass |
| code review (`low`) | 1 finding, fixed |

red: none (docs and data)

Code review: 1 findings
- `ROADMAP.md`: the Why line kept the 51% of 2026-10-28 beside the new baseline; now 47% of 2026-11-03 (fixed).
