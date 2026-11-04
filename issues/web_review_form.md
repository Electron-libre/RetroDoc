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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
Show the review of each domain, feature and use case on its page and in the lists of its parent, and let a form sent
with `htmx` set, change or remove it through the store of `review_store`, refused from another site. Out of scope:
telling a review outdated (`review_outdated`), the debt and the MCP answers. Means differ on one point: the reviews
are read from the file at each request (no copy in the shared state, so no lock beyond the store's own).

## Plan
1. [x] Show the reviews: the review block on the pages of a domain, a feature and a use case, the status badge in the
   rows of the home, domain and feature pages, the error of an unreadable file, English and French texts; verified by
   tests of the router over a `reviews.json` written by `reviews::record`; makes true WEB-79, WEB-80, WEB-82, WEB-87. (autonomous)
2. [x] The form: a `POST` on the URL of each section page, the `Origin`/`Referer` check, the `htmx` fragment or the
   `303`, the 404, 400 and 500 answers; verified by router tests that post forms and read `reviews.json`, and a headless
   Chrome check of the swap; makes true WEB-12, WEB-81, WEB-83, WEB-84, WEB-85, WEB-86, WEB-88; sets the spec
   `accepted`. (autonomous)
3. [ ] Docs of the whole issue and fixes of its `high` review.

## Decisions
- Mode: autonomous
- Base commit: 0abf63aacd1f93ce6d0448e5ab1b1c4c5763b65e
- The form posts to the URL of its page (`POST /domains/<domain>[/<feature>[/<use case>]]`), not to a route of its
  own: a static segment such as `/review` would shadow a feature or a use case of that slug.
- A `POST` without `htmx` gets a `303` to the page, so the form works without JavaScript.

## Deliverable 1
- done: the pages of a domain, a feature and a use case show their review after the confidence (or that none is
  given, or the error of an unreadable `reviews.json`); the rows of the home, domain and feature lists carry the
  status badge; the reviews are read at each request; English and French texts.
- red: WEB-79, WEB-80, WEB-82, WEB-87: `no review block in <!doctype html>…` (every test of `review_pages.rs`).
- checks: tests, clippy, specs pass (`cardinal run project-check`); docs: `docs/crates/render-mcp-cli.md`.
- Code review: 2 findings
  - Feature and use case keys carry no domain: not changed, the features pass makes slugs unique across domains
    (`features/from_surface.rs`) and REV-1 defines the key.
  - The home page drops the error of an unreadable `reviews.json` (no badge): not changed, WEB-87 names the error on
    the pages of the sections, which the home page links to.
- not verified: the badges in a browser (the headless check comes with the form, deliverable 2).

## Deliverable 2
- done: a form under the review of each section posts to the URL of its page, records or removes the review through
  the store, answers the review alone to `htmx` and a `303` otherwise; every request but `GET` and `HEAD` is refused
  unless its `Origin` (else `Referer`) is the server itself; 404, 400 and 500 answers; the spec is `accepted`.
- red: WEB-12, WEB-81, WEB-83, WEB-84, WEB-85, WEB-86, WEB-88: `left: 405` (no `POST` route), `no form` in the review
  block.
- checks: tests, clippy, specs pass (`cardinal run project-check`); docs: `docs/crates/render-mcp-cli.md`,
  `README.md`, the doc of `commands::ui::run`; headless Chrome on a copy of a saved linkding run: the swap shows
  "Wrong by <user.name>, <date>" in place, the home card gets the badge, `reviews.json` holds the review; two
  cross-site `POST`s with `curl` (foreign `Origin`, none) got 403.
- Code review: 2 findings
  - A failed `htmx` save swapped in the error without the form: fixed, the form stays, holding the comment sent.
  - A 404 to `htmx` swapped the whole not-found page into the section: fixed, it gets the error fragment.
  Both covered by `a_failed_htmx_post_gets_the_error_and_the_form_with_its_comment`.
- not verified: the status chosen is not kept by a failed save (only the comment); the headless check ran before
  these two fixes, the router tests cover them.
