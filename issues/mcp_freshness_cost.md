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
