# Let a generated name stand for several reference items in the benchmark

# Goal
A generated domain or feature can only stand for one reference item, so the judged recall under-counts on
repositories where the model merges two capabilities into one feature. Count a merged feature for each reference
item it really holds, so the recall of an application does not depend on how the model groups capabilities.

# Findings (code review `high` of `second_application_in_benchmark`, 2026-10-27, Lobsters, deepseek-chat)
* `matching::score` keeps the first hand-written pair of a generated name; a second pair of the same name is
  ignored, so `matches.yaml` cannot say it either.
* `judge` only reads the generated and reference names left unmatched (`judge.rs`, `unmatched_generated`,
  `unmatched_reference`), and proposes one reference item per generated name.
* On the complete run of Lobsters, "Voting and flagging" and "Story voting, flagging, hiding and saving" were paired
  with "Upvote stories and comments" only; "Flag stories and comments" and "Hide and save stories" stayed
  uncovered although these features hold them.

# Approach
1. Measure first, without LLM: on the saved `judge.yaml` of the lobsters run and of a linkding run, count by hand
   the generated features that hold more than one reference feature, and the recall they would add.
2. If the gain is above the spread between runs: let a pair list several reference items (`matches.yaml`), and
   let the judge propose several for one generated name; precision keeps counting the generated name once.
3. Otherwise, close it with the count, and say in skill `quality-benchmark` that the recall is a floor when the
   model merges features.

# Resources
* `crates/retrodoc-pipeline/src/benchmark/matching.rs` (`score`), `judge.rs`
* `benchmark/lobsters/matches.yaml` (header: superset pairs), skill `quality-benchmark`
* The ledger of measures: the lobsters run `second_application_in_benchmark-d8d8c92-1` keeps its `judge.yaml`

# Hints
* A change of the scoring makes the earlier figures of the ledger incomparable: rerun no paid measure for it,
  re-score the saved runs.

# Tracking

Plan: validated

## Understanding
Count, without LLM, how much judged feature recall the one-pair-per-generated-name rule loses, then either let a pair
hold several reference items (scoring and judge) or close with the count and a note in skill `quality-benchmark`.
Out of scope: the judge missing one-to-one pairs, wrong judge pairs, any paid rerun.

## Measure (step 1 of the approach, 2026-10-27, no LLM)
Reference features left uncovered by `matches.yaml` + the saved `judge.yaml`, read by hand against the descriptions of
the generated features (script `~/retrodoc-runs/scripts/merged-features/uncovered.py`).

| Run | Recall now | Uncovered held by a generated feature already paired | Recall with them | Gain | Spread between runs |
|---|---|---|---|---|---|
| lobsters `second_application_in_benchmark-d8d8c92-1` (deepseek, 2026-10-27) | 25/31 (81%) | 6: edit/delete story, hide and save, edit/delete comment, flag, browse by tag/domain, feed reader | 31/31 (100%) | +19 pts | features replays 84-90% (6 pts) |
| linkding `clones-1793046053/shown-1` (deepseek, 2026-10-26) | 18/24 (75%) | 2-3: save a bookmark, manage users (notes: weak) | 20-21/24 | +8 to +12 pts | 15-20 pts (G2) |
| linkding 6 hidden runs of 2026-10-15 (stale judge) | 14-19/24 | 1-3 per run (notes, read it later, merge tags, feeds, admin) | | +4 to +12 pts | 21 pts on these runs |

- `retrodoc-benchmark-lobsters/clones-1793094583/hidden-1` is the same run as the source run (same 98 features).
- The lobsters features replays saved only their table: no feature list to count on.
- On the old linkding runs most misses are not merges: a dedicated generated feature exists (reader mode, RSS feeds,
  bundles) but the judge left it unpaired or paired it wrongly.

## Plan
1. [x] Scoring: `matching::score` covers the name match and every pair of a generated name, precision counts the
   name once; `matches.yaml` of lobsters lists the superset pairs; verified by unit tests of `score`; spec
   `benchmark-scoring` committed; makes true BSC-1, BSC-2, BSC-3.
2. [x] Judge: offered every generated item against the uncovered reference items, keeps several reference items
   for one generated name; verified by `judge.rs` tests on the fake LLM; makes true BSC-4, BSC-5 (spec `accepted`).
3. [x] Re-score and record: re-score the saved lobsters and linkding runs with the new pairs (no LLM); the ledger
   says the feature figures before this change are not comparable; skill `quality-benchmark` says a merged feature
   counts for each reference item it holds; verified by `just check`.

## Decisions
- Measure: implement (branch a), the gain on lobsters is 3 times its spread (2026-10-27).
- Mode: autonomous
- Base commit: 075ffb35e8b346a3ff17168dc3781b26d60e59d0

## Deliverable 1 (scoring)
- done: a generated name covers its name match and every pair; precision counts it once; 8 superset pairs in
  `benchmark/lobsters/matches.yaml`; doc in `docs/crates/pipeline.md` and skill `quality-benchmark`.
- checks: `just check` ok, `just test-harness` ok, specs ok (BSC-1 to 3 traced), code review low: no findings.
- red: `a_pair_adds_to_the_name` left `[("Billing", "Payments")]`, right with `("Billing", "Billing")`;
  `a_merge_counts...` recall `Some(0.25)` instead of `Some(0.75)`.
- not verified: the re-score of the saved runs (deliverable 3).

## Deliverable 2 (judge)
- done: the judge is offered every generated item (the uncategorized bucket out) against the uncovered reference
  items, may pair one generated name with several, drops repeated pairs only; prompt says so; spec `accepted`.
- checks: `just check` ok, `just test-harness` ok, specs ok (BSC-1 to 5 traced), code review low: no findings.
- red: `keeps_several_reference_items...` kept `[Billing -> Payments]` only; `offers_every_generated_feature...`
  proposed `[]` (Checkout, name-matched, was not offered).
- not verified: what the model answers with the new prompt (a paid judge run, deliverable 3 asks first).

## Deliverable 3 (re-score and record)
- done: 2 superset pairs in `benchmark/linkding/matches.yaml`; saved runs re-scored by
  `~/retrodoc-runs/scripts/merged-features/rescore.sh` (no LLM: `retrodoc benchmark`, then again with `--matches`
  on `matches.yaml` + the saved `judge.yaml`; `retrodoc benchmark` alone never reads `judge.yaml`); ledger line added; skill `quality-benchmark` says
  older feature recalls are not comparable.

| Run | Features strict before -> after | Features judged before -> after | Precision judged |
|---|---|---|---|
| lobsters d8d8c92-1 | 81% -> 100% | 81% -> 100% | 51% (unchanged) |
| linkding 10-26 shown-1 | 12% -> 21% | 75% -> 83% | 33% (unchanged) |

- checks: `just check` ok, `just test-harness` ok; code review low: no findings.
- not verified: a new judge call with the new prompt (paid, not run); lobsters at 100% saturates on features,
  its domains (43%) still separate settings.

## Deliverable 4 (fixes of the `high` review of 075ffb3..HEAD)
| Finding | Done |
|---|---|
| a pair can no longer undo a match by equal names | kept (validated at the plan stop); spec non-goal says so |
| the judge may pick a broad matched feature over a dedicated unmatched one | prompt: a dedicated item is paired too |
| `(N matched)` counted pairs | counts the generated items, `(N matched, M pairs)` when they differ; test |
| judge prompt lists every generated feature (tokens) | not fixed: about 98 short descriptions on lobsters, cents |
| `judge` repeats the selection of generated items of `compare` | not fixed: refactor, no behaviour at stake |
| no scoring version in `RunReport` | new issue `scoring_version_in_benchmark_reports`, ranked 2nd |
| issue still in the queue | closing asked to the user at the end |
| the re-score wording named `retrodoc benchmark` alone | reworded: the script and `--matches` |

- checks: `just check` ok, `just test-harness` ok; code review low: no findings.
- red: `a_merged_item_counts_once_in_the_matched_count` printed `(2 matched)`.
