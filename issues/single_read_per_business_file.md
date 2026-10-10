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
