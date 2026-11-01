---
priority: medium
depends_on: [web_ui_feature_and_use_case_pages]
why: 'checking a claim against the code is what makes the docs trusted, after the pages exist'
---
# Show the cited code and the docs of each file in the web interface

# Goal

From a step, a rule or a feature of the web interface, open the code it cites at the cited lines, with the recent
commits of the file; and the other way round, from a file or a directory of the repository, see the use cases,
features and rules that cite it, and which of its files no doc cites.

# Approach

1. A code page served through `SourceAccess` (only the cited files, the same refusals and windows as `read_source`),
   the cited lines highlighted, the commits of `git_log` beside it, the stale warning when the file changed since
   the run.
2. A files page: the tree of the source files of the last run, each file with the number of docs that cite it, a
   directory with the share of its files cited; a file opens the list of what cites it.
3. Router tests: a cited file at its lines, a file outside the cited ones refused, a directory with uncited files.

# Specs

* `specs/web-ui.md` amended.

# Milestone

Browse the docs in a web interface

# Resources

* `crates/retrodoc-mcp/src/source.rs` (`SourceAccess`), `crates/retrodoc-mcp/src/tools/read_source.rs`,
  `crates/retrodoc-mcp/src/tools/git_log.rs`
* `docs/adr/0016-read-only-llm-free-mcp-server-for-agents.md` (why the code access is narrow)
* `issues/git_log_follow_renames.md`

# Hints

* Never widen what `SourceAccess` serves for the web: once hosted (ADR 0039), the interface is reachable by others.
* A syntax highlighter is a comfort; plain text with line numbers is enough for this issue.
