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
   inside the repo to an outside directory, `./a.rb` still read) and a `check_step` test citing the absolute path of
   a file outside the repo that comes back `Broken`; ~80 lines.

2. [x] Fixes of the `high` review: the safety-net line of `ARCHITECTURE.md` says what the guard guarantees, the
   plan line names the test it has; the gitignored-file gap goes to `issues/done/artifact_reads_stay_in_the_walked_files.md`.

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

## Review `high` (`9ab490d..f6016a6`): 9 findings
| finding | what was done |
|---|---|
| gitignored files inside the repo still readable | new issue `artifact_reads_stay_in_the_walked_files` (outside this issue's goal) |
| race between `canonicalize` and `read` | not fixed: whoever can swap a file in the repo can already put any content there |
| root canonicalized at each read | not fixed: a few syscalls next to reading the file; the CLI already canonicalizes it once |
| copy of the MCP check, other rules | not fixed: the MCP refuses every link and returns sentences for the agent, a deliberate difference |
| `canonicalize` may fail on some Windows drives | not fixed: the CLI already canonicalizes the root, so such a drive fails before |
| a `..` staying inside is refused | not fixed: decided in the issue; LLM citations are matched to the shown files before |
| `?` callers stop a pass on a refused path | not fixed: same as a missing file before this change, paths from the walk or the role map |
| hard link to an outside file passes | doc wording fixed (a path can't point outside); a hard link is content put in the repo |
| plan says the test cites `/etc/...` | fixed |

## Deliverable 2 summary
- done: `ARCHITECTURE.md` wording, plan line, new issue queued second.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | n/a |
| docs | `docs/ARCHITECTURE.md` §4 |
| code review | low: no findings |

- red: none (docs and issues only).
- not verified: nothing.
- Code review: no findings
- decisions needed: none.
