# Keep every source read inside the analysed repository

# Goal
`text::read_file_lossy` joins the repository root with a path that may come from a saved artifact. An absolute
path or one with `..` reads a file outside the repository, whose content can then be sent to the LLM. Reject such
paths in the one helper every pipeline read now goes through.

# Findings (code review `high` of `issues/done/shared_lossy_read_and_one_pass_truncation.md`, 2026-10-29)
* `Path::join` with an absolute path drops the root: `read_file_lossy(root, "/etc/passwd")` reads `/etc/passwd`.
* `confidence/checks.rs` (`check_step`) and `confidence/mod.rs` (cited excerpts) read the paths cited by the saved
  use cases, a YAML artifact that a user or another tool can edit; the excerpt goes into the confidence prompt.
* `retrodoc-mcp/src/source.rs` already resolves a path and checks it stays in the repository (`real`): a model to
  follow or share.

# Approach
1. Reject in `read_file_lossy` a path that is absolute or has a `..` component (a `PipelineError` the callers
   already handle as unreadable); decide whether symlinks leaving the repo are rejected too (canonicalize, as the
   MCP does, costs a syscall per read).
2. Add a test per case (absolute, `..`, symlink if decided) and check the citation parsing does not already
   normalise these paths away.

# Resources
* `crates/retrodoc-pipeline/src/text.rs`, `confidence/checks.rs`, `confidence/mod.rs`, `citations.rs`,
  `crates/retrodoc-mcp/src/source.rs`.
