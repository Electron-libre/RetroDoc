---
priority: medium
depends_on: [edit_store, edits_in_read_model]
why: 'the milestone''s entry point: without the form nobody can edit a section'
---
# Edit a section from its page in the web interface

# Goal
The page of a domain, a feature or a use case shows the docs and takes a review, not a correction. Expected: a form
on the page edits its name, description and narrative, saved through the store of `edit_store`; the page says the
section was edited by a human (author, date), shows the generated text it replaces, and brings it back on request.

# Findings
* 2026-11-05: the review form (`crates/retrodoc-web/src/review_form.rs`) already posts to the URL of the page with
  `htmx`, refuses a cross-site `POST` (`refuse_cross_site`, WEB-84) and answers a fragment to `htmx`, a redirect
  otherwise.
* 2026-11-05: `specs/web-ui.md` lists "no edit of the docs" as a non-goal: this issue amends it.

# Approach
1. An edit form beside the review form, the fields filled with the text shown; a `POST` saves the edit, an empty
   field or "back to the generated text" removes its part of the edit.
2. The mark "edited by a human" on the page and in the lists of sections, and the generated text folded under it.
3. The same checks as the review form: same origin, a section of the run, a store that can't be read.

# Specs
* `specs/web-ui.md`: the edit form, the mark of an edited section; amend the non-goal.

# Milestone
Edit the docs

# Resources
* `crates/retrodoc-web/src/review_form.rs`, `review.rs`, `templates/review.html`, `specs/web-ui.md` WEB-79 to
  WEB-90.

# Hints
* `htmx` keeps the swap of an error answer only through the `before-swap` hook of the review section: a fragment
  must start with its `<section`, whitespace included.
