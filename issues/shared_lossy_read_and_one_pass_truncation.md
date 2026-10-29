# Read source files through one helper and cut long texts in one walk

# Goal
Since `issues/done/remove_repo_map.md`, `crate::text` holds `read_file_lossy` and `truncate_chars`, shared by the passes
that read files. Several modules still read a file and decode it by hand, and `truncate_chars` decodes a whole text
to cut its head. Make every lossy read of a source file go through one helper and cut a text in one bounded walk,
with no change of behaviour.

# Findings (code review `high` of `issues/done/remove_repo_map.md`, 2026-10-29)
* `std::fs::read` followed by `String::from_utf8_lossy` by hand: `source_hashes.rs` (two places, `capture` and the
  second capture function), `contexts/run.rs`, `rules/mod.rs` (hash of a file, `"unreadable"` on error),
  `confidence/mod.rs` (excerpt of a cited file).
* MCP freshness compares the hashes of `SourceHashes` with its own lossy read: the passes and the hash capture
  must keep decoding a file the same way, which several copies make easy to break.
* `truncate_chars` runs `chars().count()` over the whole text, then `chars().take(max_chars)`: a large file is
  decoded in full to keep its first few thousand characters (the actors pass, the benchmark judge, the rules
  replay).

# Approach
1. Make the hand-written reads call a helper of `crate::text` (`read_file_lossy`, or a variant that returns the
   bytes' text without the `PipelineError` where the caller maps an error to `"unreadable"` or skips the file).
2. Cut in `truncate_chars` at `content.char_indices().nth(max_chars)`: no cut when it is `None`.
3. The existing tests cover the behaviour; add one for `truncate_chars` at the boundary (exactly `max_chars`
   characters, multi-byte characters) if none exists.

# Resources
* `crates/retrodoc-pipeline/src/text.rs`, `source_hashes.rs`, `contexts/run.rs`, `rules/mod.rs`,
  `confidence/mod.rs`, `crates/retrodoc-mcp/src/freshness.rs`.

# Hints
* `retrodoc-mcp` cannot call the pipeline-private `crate::text`: keep its read as is or make the helper public.
