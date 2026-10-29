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

# Tracking

Plan: validated

## Understanding
`read_file_lossy` refuses a path that is absolute, has a `..` (or root/prefix) component, or resolves outside the
repository through a symbolic link, with the `PipelineError::Read` every caller already treats as unreadable. No spec:
an internal guard of the one read helper, no capability's behaviour changes for a path the walker yields (it does not
follow links, so no walked file is a link). The MCP keeps its own check (it returns sentences for the agent).

## Plan
1. [x] Guard `read_file_lossy`: lexical check (absolute, `..`), then canonicalize the file and the root and require
   the file under the root; verified by unit tests in `text.rs` (absolute, `..`, link to a file outside, link
   inside the repo to an outside directory, `./a.rb` still read) and a `check_step` test citing `/etc/...` that
   comes back `Broken`; ~80 lines.

## Decisions
- Mode: autonomous
- Base commit: 9ab490db8f4954ea21a374ae93107236003885b0
- Links: canonicalize, refuse what resolves outside the repo; a link inside it stays readable (1 a).

## Deliverable 1 summary
- done: `read_file_lossy` resolves the path through `resolve_inside_repo` and refuses, as `PipelineError::Read`
  (`InvalidInput`), an absolute path, a `..` and a link resolving outside the repo; `ARCHITECTURE.md` safety nets.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | n/a (no spec; existing warnings only) |
| docs | `docs/ARCHITECTURE.md` §4 safety nets |
| code review | low: no findings |

- red: none (no spec): 4 tests failed on `assert!(...is_err())`, the outside file was read.
- not verified: Windows (link tests are `cfg(unix)`; the lexical check covers prefixes there).
- Code review: no findings
- decisions needed: none.
