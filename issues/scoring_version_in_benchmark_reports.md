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
