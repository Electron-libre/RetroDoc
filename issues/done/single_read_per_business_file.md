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
* `issues/done/business_rules.md` will add rules to the same reading: leave room for them in the answer type.

# Tracking

1. [x] Overlap measure (example `read_overlap`, no LLM): files read by both the glossary and the entry points passes, characters read twice, on saved runs; decides whether the rest is worth it.
2. [x] Joint reading (`business_read/`): one prompt and one answer type (entities and entry points with outputs, room for `rules`), `BatchedRead` with 5,000-character chunks over the business files, results written to `glossary.yaml` and `entry-points.yaml` with the same per-file hash as the two passes, called by `generate` before them; `glossary` and `entry-points` commands unchanged.
3. [x] Full measure on linkding and `delivery_router` (local model, three runs per variant): calls, tokens, time, `entry_points_replay` recall, entities found; joint reading stays opt-in if recall drops beyond the noise.
4. [x] Docs (`update-docs`) and a short ADR.

## Decisions

* One chunk size for the joint reading: 5,000 characters (the entry points one, which risks losing actions if cut shorter).
* Only the overlap (the business files) is read jointly; `entrypoint` files outside the business list and `model` files without a business list keep their own pass and prompt.
* The joint reading writes the two existing artifacts with the same per-file hash, so `build_glossary` and `build_entry_points` find them up to date and later passes do not change; a file the joint reading fails on is read by each pass as before.
* The standalone commands `glossary` and `entry-points` stay as they are; the joint reading runs in `generate` only, in both `domains_from` modes.
* Same measure protocol as `body_lines`: local model, three runs per variant, compared with `entry_points_replay`.
* Step 1 measured (`read_overlap` example, saved runs, characters of the files on disk): on linkding all 13 files the glossary reads are also read by the entry points pass (35 files), 101k of 277k characters, 36% of the input of the two passes; on `delivery_router` 8 of 9 files, 49%. The Rails test repository run predates the business files (the glossary read the `model` files, none shared): not measured there. Counts input characters only, not the fixed part of the prompts or the calls.
* Step 3 measured on linkding (local qwen3.6, three runs per variant; separate = `entry-points` then `glossary`, joint = `business-read` then the two passes, which still read the 22 entry point files outside the business list). Calls and tokens in per run (prompts are the same every run): separate 18 + 10 = 28 calls, 52.8k + 27.9k = 80.7k; joint 10 + 8 + 0 = 18 calls (-36%), 31.5k + 22.5k = 54.0k (-33%). Time of the two passes: separate 8.4, 9.6, 9.1 min, joint 7.6, 6.2, 7.6 min. Entry points, groups of the reference found: separate 86, 86, 79%, joint 93, 93, 79%; entries in reference files separate 74, 68, 81%, joint 75, 78, 84%. Glossary names / attributes / associations: separate 14, 16, 12 / 118, 132, 103 / 23, 18, 20; joint 19, 20, 16 / 103, 161, 73 / 11, 26, 12. No loss of recall measurable above the noise on this web application. On the small Ruby library `delivery_router` the joint reading is worse: 62% of the groups in three runs (5-6 entries) against 50-100% and 7-22 entries separate, and a first prompt that listed entities before the entry points found only 1 entry (12%): the order in the prompt and in the answer matters. Two glitches seen: the model copies the `--- path ---` header as the file name (login entry points dropped, in the separate passes too; the attribution now ignores the dashes), and entities cited under a file that was not in the batch are dropped.
* Decision after the measure (user, 2026-10-12): the joint reading stays the default of `generate`, with `[pipeline] business_read = false` to turn it off; ADR 0028. Also added `retrodoc business-read`, a standalone command (needed to measure without a whole `generate`).
