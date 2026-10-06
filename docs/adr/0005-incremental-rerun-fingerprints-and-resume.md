# 0005. Incremental re-run by input fingerprints, resumable passes

Status: Accepted

_Retroactive ADR, reconstructed from the history (f3894db, adc956b, 4e79e0d, ba23e19, 3843170, cab7929; 2026-09-28 to 2026-10-02)._

## Context

Token cost and run time are the main risks on a real repo (a local model needs ~30 min for 26 files).
Early smoke tests showed two problems: a transient LLM failure midway discarded every summary of the
run, and the non-deterministic domain clustering changed slugs on each run, so a "no change" re-run
redid everything (8 min instead of ~75 s), since downstream fingerprints are keyed by slug.

## Decision

Every pass skips a unit whose input fingerprint is unchanged and reuses its saved result: file
summaries by content hash, directory summaries by the hash of their children listing (a changed file
invalidates only itself and its parents), domains by a hash of the clustering input (paths, summaries,
existing docs; regenerated directory summaries excluded), features per domain unit, use cases per
feature. `fingerprints.rs` and `cache.rs` hold this. Passes persist after every LLM call or batch
(inventory, glossary, repo map) and save the units done so far when a pass fails (features, use cases),
so a rerun resumes. `generate --force` wipes the caches; `--dry-run` previews. A single global
"has anything changed" check was rejected: one edited file would redo everything.

## Consequences

A no-op rerun costs no LLM call and renders no diff. Any new pass must define its fingerprint, and
changing a prompt does not invalidate caches by itself (use `--force`). Saving after each call writes
more often but loses nothing on failure.

A feature the LLM answered "no use case" for, cleanly on both attempts, is remembered like any other result
(its fingerprint is saved without use cases), so a rerun on an unchanged input does not ask again. The
cost: a bad model draw stays frozen until the input changes or `generate --force`. Unparseable answers and
LLM errors are not remembered and stay retried (resume).
