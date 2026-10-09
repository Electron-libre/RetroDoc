# Fix the false "fresh" after an interrupted generate

# Goal

Make the MCP freshness warning right even after a `generate` that failed halfway. Today a file edited after
the docs were generated looks fresh if a later `generate` got past the repo map and then failed: the repo map
cache already holds the new hash, while the features and use cases (and the rendered docs) still describe
the old code. An agent is then told the docs are fresh when they are not, which is the failure the warning
exists to prevent.

# Approach

First reproduce it with a test (fake LLM that fails in the features or use cases pass, then check what
`Freshness` says), and measure how often it can happen: only a rerun that fails after the repo map pass, and
only until the next successful `generate`.

Then pick the least invasive fix. Candidates:

1. Record the hash of each cited file when the docs are produced, not when the repo map is: a `sources.json`
   artifact (or a field in `use-cases.yaml` and `features.yaml`) written at the end of the use cases pass, and
   read by `Freshness` instead of `repo-map.json`.
2. Record the hashes at render time next to `run-metadata.json`, so they describe what is on disk in `docs/`.
3. Compare against the git commit of the last render (`run-metadata.json`). Weaker: it misses uncommitted
   changes present at generation, and `retrodoc render` alone rewrites the commit with the current `HEAD`.

# Resources

* `crates/retrodoc-mcp/src/freshness.rs` and `Docs::stale_files` in `tools.rs` (the current check)
* `crates/retrodoc-pipeline/src/cache.rs` (`RepoMapCache::content_hash`), `fingerprints.rs`, `artifact.rs`
* ADR `0005` (incremental re-run, fingerprints and resume)
* `PLAN.md` §7.3 (freshness requirement) and `issues/mcp_server.md` (deliverable 3 and its decision)

# Hints

* The per-unit fingerprints of `fingerprints.json` can't be reused: they mix the feature text, the actors and
  the entry points with the file contents, so the MCP crate can't recompute them.
* A new artifact must go through `Artifact` (name, `generate --force` behavior) and the `update-docs` checklist.
* Existing generated repos have no such artifact: a rerun of `generate` (no LLM call when nothing changed)
  should be enough to write it, and `Freshness` should keep working, with `Unknown`, until then.

# Tracking

1. [x] `source-hashes.json` artifact and `Freshness` reads it (reproduction test first; no file = `Unknown`)
2. [x] `generate` captures the hashes before the repo map and saves them only after a successful run; `--force` clears them
3. [x] Docs (`update-docs` checklist: `CLAUDE.md`, `PLAN.md` §7.3, `issues/mcp_server.md`, doc comments)

## Decisions

* Option 1 of the Approach (the hashes are recorded when the docs are produced, not by the repo map).
* The artifact holds the hash of every source file of the repo map (not only the cited ones).
* Hashes are read before the repo map pass and written only once the whole run succeeded, so an edit made
  during the run reads as modified (the safe side).
* `generate --force` clears it; until a run ends, every file is `Unknown`.
* Named `source-hashes.json` (not `sources.json`, too close to `signal-sources.yaml`).
