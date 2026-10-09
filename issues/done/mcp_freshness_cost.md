# Keep the cost of the MCP freshness check in view

# Goal

Make sure the freshness warning of the MCP server stays cheap on a large repository. Every `get_feature`,
`get_use_case` and `search_docs` answer currently re-reads and re-hashes each file it cites. That is
negligible on a small repo, but a search with many hits, or a feature citing many large files, on a big
repository multiplies the file reads per call, and an agent often makes ten calls or more per question.

# Approach

Measure before optimizing: on a large generated repo, time the three tools with and without the check (number
of files read, bytes hashed, milliseconds per call). Only if it matters, choose among:

1. Skip the content hash when the file's size and modification time are the same as at the last check, and
   keep a small in-memory table of those (the server is long-lived).
2. Cache the verdict of a file for a short delay, accepting that an edit is noticed a few seconds later.
3. Limit the check to the first N hits of a search, saying so.

# Resources

* `Docs::stale_files` and `stale_notice` in `crates/retrodoc-mcp/src/tools.rs`
* `crates/retrodoc-mcp/src/freshness.rs` (`Freshness::state`, `stale`)
* `issues/mcp_server.md` (deliverable 3, decision on the hashes of `repo-map.json`)

# Hints

* Don't trade correctness for speed: a changed file must still be reported, including by a server that has
  been running for hours. An mtime-only shortcut can miss an edit that keeps the same size and timestamp.
* Reading the files at call time is deliberate: it is what lets the server notice an edit made meanwhile.
* If the fix for `issues/stale_docs_after_failed_generate.md` changes where the recorded hashes come from,
  do this one after it.

# Tracking

1. [x] Measure the cost of the freshness check (`#[ignore]` test on a synthetic repo, figures noted here)
2. [x] (not needed, see Measure) Only if the measure crosses the threshold: skip the hash when size and mtime are unchanged and the mtime is older than the last check
3. [x] Docs: n/a (no behavior change, no optimization)

## Decisions

* Measure on a synthetic repo (reproducible); the smoke-test repo is too small to show anything.
* Threshold to optimize: more than 50 ms per call on a realistic case. Below it, close the issue with the figures.
* Option 1 only (size + mtime), trusted only when the mtime is older than the last check; options 2 and 3 are out (they delay or skip detection).

## Measure

`cost_of_the_check` in `crates/retrodoc-mcp/src/freshness.rs` (`#[ignore]`, release build, synthetic repo of
2,000 files, warm page cache, so the disk is not in the figures; `FRESHNESS_FILE_KB` sets the file size).
Milliseconds per call, only the check (without it the tools read no file):

| Call | 4 KiB files | 20 KiB files | 200 KiB files |
|---|---|---|---|
| `get_feature`, 5 files | 0.03 | 0.19 | 0.74 |
| `get_feature`, 50 files | 0.23 | 1.57 | 7.0 |
| `search_docs`, 10 hits x 5 files | 0.18 | 1.53 | 6.9 |
| `search_docs`, 20 hits x 20 files (the cap, `MAX_LIMIT`) | 1.8 | 13 | 56 |

The worst case is 400 files of 200 KiB (80 MB hashed in one call), far above a realistic feature. Realistic
calls stay under 2 ms. Below the 50 ms threshold: no optimization, and none of the options 1 to 3 is kept.
Not measured: a cold disk cache (the first read of a file after a long idle).
