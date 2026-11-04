---
priority: medium
depends_on: [review_store, review_in_debt]
why: 'a review of a section that changed since must not keep vouching for it'
---
# Flag a review whose section changed since it was given

# Goal
A review is given on a section as the run wrote it. A later `generate` can rewrite the section, and the code it
cites can change. Expected: such a review is kept but shown outdated, and the section is back in the debt as the
model judges it (decided with the user on 2026-11-04).

# Findings
* 2026-11-04: the stale sections (DBT-7) are judged at each request from the hashes of the cited files the run
  saved; `retrodoc_mcp::freshness` does it.

# Approach
1. Outdated when the fingerprint `review_store` saved differs from the section's now, or when a file the section
   cites changed since the review.
2. The pages, the debt and the MCP answers show the review outdated; a validated section that is outdated no longer
   leaves the debt of judgement.

# Specs
* `specs/section-review.md`: when a review is outdated.
* `specs/debt-view.md`: an outdated validation.

# Milestone
Human review of the docs

# Resources
* `crates/retrodoc-mcp/src/freshness.rs`, the store of `review_store`.
