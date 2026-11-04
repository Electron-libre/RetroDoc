---
priority: medium
depends_on: [review_store]
why: 'an agent reading the docs must know what a human checked or rejected, like a reader of the pages'
---
# Show the human review of a section in the MCP answers

# Goal
The MCP tools answer with the confidence of the model only. Expected: `get_domain`, `get_feature` and
`get_use_case` say the review of the section (status, comment, author, date), and warn above the answer when a human
marked it wrong.

# Findings
* 2026-11-04: the tools answer in Markdown from `Docs` (`crates/retrodoc-mcp/src/tools/`), the freshness warning of a
  cited file already sits above the answer.

# Approach
1. Read the reviews with the docs; a line of the review under the confidence of the section.
2. A section marked wrong: a warning above the answer, like the one of a stale cited file.
3. `list_domains` unchanged.

# Specs
* `specs/mcp-server.md`: the review in the answers.

# Milestone
Human review of the docs

# Resources
* `crates/retrodoc-mcp/src/tools/` (`answer.rs` for the pieces the answers share), `specs/mcp-server.md`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
`get_domain`, `get_feature` and `get_use_case` show the review of the section under its confidence, and open with a
warning when it is marked wrong; reviews read from `.retrodoc/reviews.json` at each answer, like the web pages.
Out of scope: `list_domains` and the listed items, outdated reviews (`review_outdated`).

## Plan
1. [x] Review in the detail answers: a review line and a wrong warning built in `answer.rs` from the reviews loaded at
   each answer (the repo root of the docs), wired in the three tools; verified by tests in `tools/tests.rs`; makes
   true MCP-7, MCP-8, MCP-9, MCP-10, MCP-11. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 4527708ef233ecf3b8f83113704c08f6cf0e6388
- The wrong warning opens the answer, before its title (MCP-8).
- No review in the items listed inside an answer (non-goal of the spec).

2. [x] Fixes of the `high` review of the issue: the three detail tools off the async threads, the date of a review
   on one line, the server instructions name the review, the spec's non-goals one list; verified by the MCP-11 test.
   (autonomous)

## Summary of deliverable 1
- done: `get_domain`, `get_feature`, `get_use_case` give the review line under the confidence and open with a warning
  when marked wrong (`tools/review.rs`); reviews read at each answer; spec MCP-7 to MCP-11 accepted.
- red: MCP-7, MCP-8, MCP-9, MCP-10, MCP-11: the answers held no `Review:` line nor warning (`# Billing (`billing`)…`).
- Code review: no findings.
- not verified: through a live `retrodoc mcp` session on a real repo (the tool functions are tested directly).

## Summary of deliverable 2 (`high` review of the issue)
- done: fixes below; check green.
- red: MCP-11: a `reviewed_at` holding a newline forged a line (test extended before the fix).
- Code review: 8 findings (high, issue).
  - fixed: blocking I/O in the async handlers (`blocking()`), `reviewed_at` not on one line, doc comment of
    `review_note`, server instructions silent on the review, blank line splitting the spec's non-goals.
  - not fixed: a review shown whatever its fingerprint (`review_outdated` does it, out of scope here); the absolute
    path in the error of an unreadable file (local stdio server, its client runs on the same host and gave `--path`;
    the web page shows the same error); `reviews.json` read at each answer (small file, MCP-9 asks for it).
- Code review (low, deliverable): no findings.
