# Read only the walked files from a path a saved artifact cites

# Goal
`text::read_file_lossy` now refuses a path that leaves the repository, but a path edited in a saved artifact can
still name a file inside it that the walker skips (a gitignored `.env`, `config/master.key`, `.git/config`); its
content then goes into a prompt sent to the LLM. A path that comes from an artifact should only reach a file the
walker yields, as the MCP only reads cited paths.

# Findings (code review `high` of `issues/source_reads_stay_in_the_repo.md`, 2026-10-29)
* The new guard checks that a path is plain and resolves under the root, not that it is a walked file.
* `confidence/checks.rs` (`check_step`) and `confidence/mod.rs` (cited excerpts) read the `source_refs` of the
  saved use cases, a YAML artifact a user or another tool can edit.
* Other callers read paths from the walk or the saved role map; whether the role map is a path in is not checked.

# Approach
1. List the callers whose paths come from an editable artifact rather than from the walk of this run.
2. Candidates (can be combined):
   1. check those paths against the walked file set before reading (the set the run already has);
   2. refuse in `read_file_lossy` a path the walker's ignore rules exclude (costs an ignore match per read).
3. A test per caller: an artifact citing a gitignored file inside the repo is treated as unreadable.

# Resources
* `crates/retrodoc-pipeline/src/text.rs`, `confidence/checks.rs`, `confidence/mod.rs`.
* `crates/retrodoc-ingest/src/walker.rs` (ignore rules, `.git` excluded).
* `crates/retrodoc-mcp/src/source.rs` (`check_cited`).
