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

# Tracking

Plan: validated

## Understanding
A `/code/<path>` page (cited files only, through `SourceAccess`: lines, cited range marked, last commits, what cites
it, stale flag) linked from steps, source files and code evidence, and a `/files` tree with the citation counts.
Out of scope: syntax highlighting, renames in the history, any content of an uncited file.
Means: `SourceAccess` returns structured windows, commits and refusals (the MCP answers unchanged) so the web lays them out.

## Plan
1. [x] Structured `SourceAccess`: a `Window` of numbered lines, the `FileCommit`s and a `SourceError` enum, the
   Markdown of `read_source`/`git_log` built from them; the set of cited paths moved to `GeneratedDocs`; verified by
   the existing `source.rs` and MCP tool tests unchanged; pure refactor (~250 lines).
2. [x] Code page `/code/<path>` and the links to it; verified by router tests (cited file at its lines, uncited and
   `..` paths 404, symlink refused, history, stale, citing items); makes true WEB-26 to WEB-33 (~700 lines).
3. [ ] Files page `/files` and the link of every page to it; verified by router tests (tree, `1/2` share, uncited
   file flagged, unrecorded cited file listed); makes true WEB-34 to WEB-37, spec `accepted` (~450 lines).

## Decisions
- Mode: autonomous
- Base commit: fe5fa4a57b9e7b97287bb77843c5c6301d523b0b
- A cited file `read_source` refuses: status 200, a page that says why, no line of it, still its history and what
  cites it (WEB-32).
- `/files` lists the paths of every source file of the run, cited or not, never the content of an uncited one.

## Deliverable 1
done: `SourceAccess::window`/`commits` return typed data and a `SourceError`; `read_source`/`git_log` lay it out as
before; `GeneratedDocs::cited_paths` holds the cited set.
red: none (pure refactor, the `source.rs` and MCP tool tests unchanged and green).

## Deliverable 2
done: `/code/{*path}` (`code::code_page`, `code.html`) through `GeneratedDocs::source_access`; step citations, feature
source files and code evidence link to it.
red: WEB-26, WEB-27, WEB-28, WEB-29, WEB-30, WEB-32, WEB-33: status 404 for `/code/app/invoice.rb` (no route), no
`/code/` link on the feature page; WEB-31 passed before the code (the fallback already answered 404).
Code review: 4 findings
- `./` prefix not stripped before the cited check: not fixed, `SourceAccess` strips it from the cited set and from the
  path asked (`check_cited`), so `app/x.rb` matches a citation `./app/x.rb`.
- `end + CONTEXT` overflow on a huge `?lines=`: fixed (`saturating_add`), covered by the past-end test.
- `Docs::with_source_access` duplicates `GeneratedDocs::source_access`: not fixed, the MCP takes its root explicitly
  and `GeneratedDocs::new` has none.
- `?lines=x&lines=5` takes the first pair only: not fixed, no range is the documented answer to a malformed one.
