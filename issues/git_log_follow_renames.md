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
* `issues/mcp_server.md` (deliverable 4 and its decisions on `git_log`)

# Hints

* The old names are not cited by the docs, so they are not readable with `read_source`: `git_log` may show them
  as history, but nothing else should start accepting them.
* Keep the path literal (no glob), as `file_log` does today, and keep leaving merge commits out.
* Copies and files split into several are out of scope: follow only clear renames, like `git log --follow`.
* The rename threshold is git's default (50% similarity); a file renamed and heavily rewritten in the same
  commit will not be followed, which is acceptable.
* While in `file_log`, note that it walks the history until it finds `limit` commits, which can be long on a
  large repository for a file that rarely changes. Measure before bounding it.
