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
2. [x] Debt, report and audit: an outdated validation stays in the kinds of judgement, the items marked wrong or to
   review and the report say outdated, the audit counts the outdated reviews; verified by tests of `/debt`, `/audit`
   and of the binary; makes true DBT-32, DBT-37, DBT-38, AUD-18 (~350 lines). (autonomous)
3. [x] Web pages: the review of a section and the badges of the lists flagged outdated (FR/EN texts); verified by
   tests of the pages; makes true WEB-89, WEB-90 (~250 lines). (autonomous)
4. [x] MCP answers: the review line and the warning of an outdated review; verified by tests of the tools; makes
   true MCP-12, MCP-13 (~120 lines). (autonomous)
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

## Deliverable 2
done: a validated section whose review is outdated stays in the kinds of judgement; the items marked wrong or to
review say why their review is outdated (debt page, `outdated` macro); `retrodoc report` adds `(outdated: …)` to a
section marked wrong (`OutdatedReview`); the audit page counts the outdated reviews of each kind. `reviews::outdated`
now reads the repository from the docs.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (in `draft`) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: DBT-32, DBT-37, DBT-38, AUD-18: the refund still left out after its cited file changed, no "Outdated review" on
the items, no `(outdated: …)` in the report, no `Outdated:` count on the audit page.

Code review: no findings
not verified: none
decisions needed: none

## Deliverable 3
done: the review of a section on its page says why it is outdated (`ReviewView::outdated`, the `outdated` macro, now
imported by `review.html`); the badge of an outdated review in the lists of the home, domain and feature pages has an
"outdated" badge beside it (FR "relecture périmée"); saving the review again makes it up to date.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (in `draft`) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: WEB-89, WEB-90: no "Outdated review" in the review block, no `badge review outdated` in the rows.

Code review: no findings
not verified: the look of the dashed "outdated" badge in a browser (only the HTML is tested)
decisions needed: none

## Deliverable 4
done: the review line of `get_domain`, `get_feature` and `get_use_case` ends with `, outdated: <reasons>` for an
outdated review, and the warning of a section marked wrong says the section changed since; `Outdated::report` and
`reasons` shared with `retrodoc report`. The tests of MCP-7, MCP-8 and MCP-11 now write the real fingerprints. The
five specs are `accepted`.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (all five `accepted`) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

red: MCP-12, MCP-13: no `outdated:` in the review line, the warning still saying "Do not rely on it." alone.

Code review: no findings
not verified: none
decisions needed: none
