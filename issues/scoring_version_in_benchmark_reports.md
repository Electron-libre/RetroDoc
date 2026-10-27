# Refuse to compare benchmark runs scored under different rules

# Goal
`benchmark-table --previous` reports the change between two series of saved `RunReport` JSON files. The files
carry no trace of the scoring rules that produced them, so a change of the scoring shows as a change of quality.
Make the table say, or refuse, when the two sides were scored under different rules.

# Findings (code review `high` of `one_generated_name_for_several_reference_items`, 2026-10-27)
* Since da35abe a generated feature counts for each reference feature it holds (spec `benchmark-scoring`).
  Re-scoring the saved runs raised the judged feature recall from 81% to 100% on lobsters and from 75% to 83% on
  linkding, with no change of the generated docs.
* A series saved before da35abe compared with one saved after shows that rise as a gain; only the text of skill
  `quality-benchmark` and the ledger warn against it.

# Approach
1. A scoring version in `RunReport` (a constant bumped when `matching::score` or the judge changes what counts),
   missing in older files.
2. `benchmark-table` prints the change only between series of the same version, and says why otherwise.
3. Or re-score the previous series from the saved clones instead of reading their JSON (no LLM for the strict
   figure; the judged one needs the saved `judge.yaml`).

# Resources
* `crates/retrodoc-pipeline/src/benchmark/table.rs` (`RunReport`, `load_series`), `matching.rs`, `judge.rs`
* `specs/benchmark-scoring.md`, skill `quality-benchmark` ("After the run")
* The re-score script named in the ledger line of `one_generated_name_for_several_reference_items`

# Hints
* No paid run is needed: the saved series and clones are enough to test it.

# Tracking

Plan: validated

## Understanding
Stamp each saved `RunReport` with a scoring version and make `benchmark-table --previous` refuse the change
between sides whose versions differ or are unknown (approach 1 + 2). Out of scope: re-scoring inside the table
(approach 3); the ledger's `rescore.sh` re-scores saved runs and now writes a versioned JSON with `--out`.

## Plan
1. [x] Scoring version: `SCORING_VERSION` (1) next to `matching::score`, `RunReport.scoring` (`Option<u32>`,
   absent in old files), table leaves Previous/Change empty with a note under the series when the versions are not
   all equal and known; skill `quality-benchmark` and docs updated; verified by unit tests in `table.rs`; makes
   true BSC-6, BSC-7.

## Decisions
- Mode: autonomous
- Two series with no version are not compared: an unknown version is never assumed equal (answer 1a).
- No way to stamp a version by hand: runs saved since da35abe are re-scored with `--out` (answer 2a).
- Base commit: 12f39edf3566db2a5b58ed69465b141cda5a2520

## Deliverable 1 summary
- done: `RunReport.scoring` stamped with `SCORING_VERSION` (1, `matching.rs`); `benchmark-table` leaves
  Previous/Change empty and prints a note under a series whose sides differ or have no version; docs and skill
  `quality-benchmark` say so.
- red: `cannot find value SCORING_VERSION` / `no field scoring on RunReport` (compile failure of the new tests).
- checks: `just check` ok, `just test-harness` ok, specs BSC-6, BSC-7 traced.
- Code review (low): no findings. Own reread: the note told to re-score only "the previous runs" when the current
  side can lack the version too (seen on two saved linkding series); reworded to "the runs of another version".
- checked by hand: `benchmark-table` on two saved linkding series prints the note and no change.
- not verified: `benchmark-table` on the saved series of `~/retrodoc-runs` (they all have no version, so every
  change is now refused until re-scored with `--out`).
