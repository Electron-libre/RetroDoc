---
priority: high
depends_on: [review_store]
why: 'the milestone''s entry point: without the form nobody can record a review'
---
# Review a section from its page in the web interface

# Goal
The pages of a domain, a feature and a use case show the docs but take no input. Expected: each shows the review
of its section (status, comment, author, date) and a form to set or change it, sent with `htmx`, saved through the
store of `review_store`, without a restart of `retrodoc ui`.

# Findings
* 2026-11-04: the server only answers `GET`; its only guard is the check of the `Host` header against DNS
  rebinding (`refuse_foreign_host` in `crates/retrodoc-web/src/lib.rs`). A `POST` from a page of another site would
  pass it: a form needs a check of `Origin` (or of a token) against cross-site requests.
* 2026-11-04: the docs and the run history are loaded once in `Site::new`; the reviews change while the server runs.
* 2026-11-04: `specs/web-ui.md` lists "No LLM call and no edit of the docs" as a non-goal; a review records a
  judgement and leaves the docs as they are.

# Approach
1. A `POST` route per section, refused unless its `Origin` (else `Referer`) is the server's own address.
2. The reviews held in the shared state of the server behind a lock, read at each request, written through the
   store.
3. A badge of the status beside the confidence badge of the section and of its rows in the lists of its parent
   page; the form swaps the badge and the review in place.

# Specs
* `specs/web-ui.md`: new requirements of the form and of the cross-site check; amend the non-goal.

# Milestone
Human review of the docs

# Resources
* `crates/retrodoc-web/src/pages.rs`, `view.rs`, `templates/` (the section pages), `lib.rs` (the routes, the
  `Host` check).
* The ADR written by `review_store`.
* The browsers of the machine to check the form headless.
