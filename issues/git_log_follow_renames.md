---
priority: low
depends_on: []
why: 'not a priority (user, 2026-10-09): small MCP improvement, do it when nothing more valuable is left'
---
# Follow file renames in the MCP git_log

# Goal

Make `git_log` show the whole history of a file, including the commits made under its previous names. Today
`file_log` lists only the commits that touched the current path, so a file that was moved or renamed shows a
history that starts at the rename (the rename itself is listed as the commit that "added" it). An agent
checking how a behavior evolved then misses the early history, which is often the most useful part. This is
what `git log --follow` does.

# Approach

Reproduce it first with a test: a file committed under one name, renamed (with and without edits), then
edited again; the log of the new name must list all three kinds of commits, newest first.

Then follow the renames while walking from the newest commit to the oldest, keeping the path being tracked:

1. When the tracked path appears as added in a commit, run a diff of that commit against its parent over the
   whole tree with `git2`'s similarity detection (`DiffFindOptions`, renames only), and, if the file is the
   target of a rename, continue with the old path.
2. Only do that full-tree diff on commits that add the tracked path, so the cost stays close to today's
   path-filtered walk.

Show the rename in the output (for example `renamed from <old path>` on the commit that did it), so the agent
understands why the path changes.

# Resources

* `crates/retrodoc-ingest/src/git_history.rs` (`file_log`, `CommitSummary`)
* `crates/retrodoc-mcp/src/source.rs` (`SourceAccess::git_log`, the output format)
* `issues/done/mcp_server.md` (deliverable 4 and its decisions on `git_log`)

# Hints

* The old names are not cited by the docs, so they are not readable with `read_source`: `git_log` may show them
  as history, but nothing else should start accepting them.
* Keep the path literal (no glob), as `file_log` does today, and keep leaving merge commits out.
* Copies and files split into several are out of scope: follow only clear renames, like `git log --follow`.
* The rename threshold is git's default (50% similarity); a file renamed and heavily rewritten in the same
  commit will not be followed, which is acceptable.
* While in `file_log`, note that it walks the history until it finds `limit` commits, which can be long on a
  large repository for a file that rarely changes. Measure before bounding it.

# Tracking

Plan: validated

## Understanding
`file_log` follows the renames of the file it walks (git's 50% similarity, renames only, full-tree diff only on the
commits that add the tracked path) and marks the commit that renamed it; `git_log` and the web page of a file show
the mark. Spec: `specs/file-history.md` (FHI-1 to FHI-5). Old names stay unreadable; copies and splits are out.

## Plan
1. [x] Follow renames: `CommitSummary`/`FileCommit` gain `renamed_from`, `file_log` switches the tracked path on a
   rename, `git_log` and `code.html` print `renamed from <old path>`, timing of `file_log` on a large repo noted;
   verified by tests in `git_history.rs`, `source.rs` and `retrodoc-web/tests/code_page.rs`; makes true FHI-1 to FHI-5.

## Decisions
- Mode: autonomous
- Base commit: dd08d3b4ed1e65c910395d488b55b847390cd7ff
- The web page of a file shows `renamed from` too (answer 1 a).

## Summaries

### 1. Follow renames
- done: `file_log` follows renames (whole-tree diff with rename detection only on the commits that add the tracked
  path) and gives `renamed_from`; `git_log` prints `(renamed from `<old>`)`, the web page of a file too; spec
  `specs/file-history.md` accepted; ADR 0016, ARCHITECTURE, crate docs updated.
- red: FHI-1, FHI-2, FHI-5: the history of `app/new.rb` stopped at `move and edit invoices` (2 commits of 5), no
  commit marked. FHI-3 and FHI-4 already held (copies are not renames, old names are not cited): guard tests.
- Measure: on llama.cpp (11,898 commits), a full walk for `LICENSE` (3 commits) takes 0.34 s, a renamed file 0.40 s
  for 21 commits, the same counts as `git log --follow --no-merges`: the walk is left unbounded.
- not verified: the web test of FHI-2 was written before the code but not run red.
