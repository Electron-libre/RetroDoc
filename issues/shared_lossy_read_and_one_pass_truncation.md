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

# Tracking

Plan: validated

## Understanding
Route every whole-file lossy read of `retrodoc-pipeline` through `text::read_file_lossy` (callers that skip a file or
hash `"unreadable"` take `.ok()` / `map_or_else` on it: no new variant), and cut `truncate_chars` at
`char_indices().nth(max_chars)`. Out of scope: `rules/input.rs` (decodes a byte prefix only), `retrodoc-ingest` and
`retrodoc-mcp` (cannot call the crate-private helper; MCP read kept as is, per the hint). Pure refactor, no spec.

## Plan
1. [x] One read, one walk: `source_hashes.rs` (2), `contexts/run.rs`, `rules/mod.rs`, `confidence/mod.rs`, plus the
   other whole-file reads found in the pipeline (`confidence/checks.rs`, `use_cases/mod.rs`, `use_cases/prompt.rs`,
   `rules/prompt.rs`, `rules/grounding.rs`) call `read_file_lossy`; `truncate_chars` cuts in one walk; verified by the
   existing tests plus unit tests of `truncate_chars` at the boundary (exactly `max_chars`, multi-byte, one over);
   ~120 lines.
2. [x] Fixes of the `high` review: `read_file_lossy` keeps the buffer of valid UTF-8 (no second copy); verified by a
   unit test of the read (valid, invalid UTF-8, missing file).

## Decisions
- Mode: autonomous
- Base commit: f420903b32066a2ef290c374bc23608650c3851c

## Summary of deliverable 1
- done: 10 whole-file reads of the pipeline call `read_file_lossy`; `truncate_chars` cuts at `char_indices().nth`.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | clippy pedantic clean |
| specs | no change (pure refactor) |
| docs | no doc names the helpers or the hand-written reads: nothing to update |
| code review | see below |

- red: none (refactor; the 3 boundary tests of `truncate_chars` pass before and after)
- Code review: no findings
- not verified: none
- decisions needed: none

## Summary of deliverable 2 (review `high` of base..HEAD)
- done: `read_file_lossy` copies only invalid UTF-8; other findings moved to follow-up issues.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | clippy pedantic clean |
| specs | no change |
| docs | nothing to update |
| code review | see below |

- red: none (the copy is a cost, not an observable behaviour; the new read test pins the decoding)
- Code review (`high`): 6 findings
  1. second copy of valid UTF-8 in `read_file_lossy`: fixed.
  2. `business_files` strict read of test files: `issues/text_reading_leftovers.md` (a change of behaviour).
  3. warnings drop the io error: `issues/text_reading_leftovers.md` (unchanged here, pre-existing).
  4. `rules/input.rs` byte cut: `issues/text_reading_leftovers.md`.
  5. other two-walk truncations: `issues/text_reading_leftovers.md`.
  6. no path containment in `read_file_lossy`: `issues/source_reads_stay_in_the_repo.md`.
- Code review (`low`): no findings
- not verified: none
- decisions needed: none

Status: done; both follow-ups queued right after this issue.
