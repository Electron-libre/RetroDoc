---
priority: medium
why: 'every correction of the reader goes through these forms; two forms side by side slow the reading down'
---
# Review and edit a section in one interaction

# Goal
The page of a domain, a feature or a use case holds two forms, one to review the section and one to edit it, and
the user finds the pair impractical. Expected: one interaction to judge a section and correct it, so that the usual
paths (validate, mark wrong and fix, leave a note to review) take the fewest steps.

# Findings
* The page holds the review form (`specs/web-ui.md` WEB-81) and the edit form (WEB-93), each with its own submit,
  notice and error (WEB-102 to WEB-105); the edit form is folded unless asked with `?edit=1` (WEB-114).
* The debt page offers two actions per item, "Review" and "Edit" (WEB-115).
* An edit saved after a review makes that review outdated (`specs/section-edit.md` SED-26): correcting a section
  marked wrong takes two saves and leaves the review said outdated.

# Approach
1. List with the user the paths a reviewer takes and the steps each takes now (screenshots of
   `just screenshots`).
2. Candidates (can be combined):
   1. one form: the status choices first, the editable fields unfolded on "wrong", one save writing the review and
      the edit together;
   2. editing in place on the page, the verdict buttons beside the heading;
   3. one action per debt item in place of "Review" and "Edit".
3. Decide what one save writes when both change, so the review is not outdated by its own edit.

# Specs
* `specs/web-ui.md` (WEB-81 to WEB-105, WEB-113 to WEB-121 amended), `specs/section-edit.md` SED-26.

# Milestone
Read by someone taking an application over

# Resources
* `docs/dev/web-ui.md`; `crates/retrodoc-web/src/review_form.rs`, `edit_form.rs`, `review.rs`, `edit.rs`.
* Closed issues `web_review_form`, `web_edit_form`, `web_ui_form_feedback`, `web_ui_edit_live_preview`.
