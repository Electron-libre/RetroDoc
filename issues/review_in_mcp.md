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
