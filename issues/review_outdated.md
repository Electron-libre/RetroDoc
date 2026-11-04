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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
A review saves the hash of each file its section cites; it is outdated, judged at each read, when its section's
fingerprint changed (rewritten) or a saved file changed or is gone. The pages, the debt, `retrodoc report`, the audit
page and the MCP answers say so with the reasons; an outdated validation no longer leaves the kinds of judgement.
Out of scope: a debt kind of its own for an outdated validation; files a later run adds to a section.

## Plan
1. [x] Store and rule: `Review` holds the hashes of its cited files (REV-19), `reviews::outdated` judges a review
   against the docs and the files now; verified by unit tests of `reviews`; makes true REV-19 to REV-22 (~300 lines).
   (autonomous)
2. [ ] Debt, report and audit: an outdated validation stays in the kinds of judgement, the items marked wrong or to
   review and the report say outdated, the audit counts the outdated reviews; verified by tests of `/debt`, `/audit`
   and of the binary; makes true DBT-32, DBT-37, DBT-38, AUD-18 (~350 lines).
3. [ ] Web pages: the review of a section and the badges of the lists flagged outdated (FR/EN texts); verified by
   tests of the pages; makes true WEB-89, WEB-90 (~250 lines).
4. [ ] MCP answers: the review line and the warning of an outdated review; verified by tests of the tools; makes
   true MCP-12, MCP-13 (~120 lines).
5. [ ] Docs of the whole issue and fixes of its `high` review.

## Decisions
- Mode: autonomous
- Base commit: 5a3cef30e90c7726b27d327ea268299f65f793cc
- A review saves the hash of each file its section cites when given (1 a).
- No debt kind of its own for an outdated validation (2 a).
- The audit page counts the outdated reviews (AUD-18, 3 a).

## Deliverable 1
done: a review saves the hash of each file its section cites (`Review::files`); `reviews::outdated` tells a review
outdated when its section was rewritten or a hashed file was modified or deleted, judged at each read;
`freshness::file_hash` and `state_against` shared with `Freshness::state`.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (section-review, debt-view, web-ui, mcp-server, audit-dashboard in `draft`) |
| docs | `docs/crates/render-mcp-cli.md`, ADR 0044 |
| code review | see below |

red: REV-19, REV-20, REV-21: `files` empty (`left: {}`), `outdated` answering `None` for a rewritten section, a
modified and a deleted file; REV-21 (no hashes, unreadable file) and REV-22 green at once (stub answering `None`).

Code review: no findings
not verified: none
decisions needed: none
