# Read each business file once for entities, entry points, outputs and rules

# Goal

The glossary and the entry points passes read the same business files with two prompts. One call per file
(or batch) should return the entities, the entry points with their outputs and, later, the rules (ADR 0020
step 6), so the input tokens of these passes drop and the two results come from the same reading.

# Findings

* On linkding the entry points pass is 12-13 calls (0.7 min deepseek, 10.4 min local) and the glossary
  reads an overlapping set of files (`issues/behaviour_coverage_domains.md`, per-pass table).
* Both passes use `batched_read.rs` (batches of about 12,000 characters, a retry file by file, a checkpoint
  per batch) and cache per file content hash.

# Approach

1. Measure: files read by both passes on linkding and `delivery_router`, tokens spent twice.
2. One answer type (entities, entry points, outputs) with its schema; the two artifacts stay as they are
   (`glossary.yaml`, `entry-points.yaml`) so the later passes do not change.
3. Keep the per-file cache (one hash for the two results) and the standalone commands (`glossary`,
   `entry-points`).
4. Compare entities and entry points found, calls and tokens against the two-pass version (`entry_points_replay`
   example, benchmark).

# Resources

* `crates/retrodoc-pipeline/src/batched_read.rs`, `glossary/`, `entry_points/`; ADRs `0020`, `0021`.

# Hints

* A bigger answer per call can lose entry points: check the recall with `entry_points_replay` on the
  hand-made reference before and after.
* `issues/business_rules.md` will add rules to the same reading: leave room for them in the answer type.

# Tracking

1. [x] Overlap measure (example `read_overlap`, no LLM): files read by both the glossary and the entry points passes, characters read twice, on saved runs; decides whether the rest is worth it.
2. [ ] Joint reading (`business_read/`): one prompt and one answer type (entities and entry points with outputs, room for `rules`), `BatchedRead` with 5,000-character chunks over the business files, results written to `glossary.yaml` and `entry-points.yaml` with the same per-file hash as the two passes, called by `generate` before them; `glossary` and `entry-points` commands unchanged.
3. [ ] Full measure on linkding and `delivery_router` (local model, three runs per variant): calls, tokens, time, `entry_points_replay` recall, entities found; joint reading stays opt-in if recall drops beyond the noise.
4. [ ] Docs (`update-docs`) and a short ADR.

## Decisions

* One chunk size for the joint reading: 5,000 characters (the entry points one, which risks losing actions if cut shorter).
* Only the overlap (the business files) is read jointly; `entrypoint` files outside the business list and `model` files without a business list keep their own pass and prompt.
* The joint reading writes the two existing artifacts with the same per-file hash, so `build_glossary` and `build_entry_points` find them up to date and later passes do not change; a file the joint reading fails on is read by each pass as before.
* The standalone commands `glossary` and `entry-points` stay as they are; the joint reading runs in `generate` only, in both `domains_from` modes.
* Same measure protocol as `body_lines`: local model, three runs per variant, compared with `entry_points_replay`.
* Step 1 measured (`read_overlap` example, saved runs, characters of the files on disk): on linkding all 13 files the glossary reads are also read by the entry points pass (35 files), 101k of 277k characters, 36% of the input of the two passes; on `delivery_router` 8 of 9 files, 49%. The Rails test repository run predates the business files (the glossary read the `model` files, none shared): not measured there. Counts input characters only, not the fixed part of the prompts or the calls.
