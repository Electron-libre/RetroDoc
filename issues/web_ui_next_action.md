---
priority: medium
depends_on: [web_ui_components]
why: 'the milestone asks each page to show what to do next; pages work without it'
---
# Show on each page the next action it offers

# Goal
A page shows its content but rarely what the reader can do next: a debt item does not offer to review or edit the
section, an empty list does not say how to fill it, the review and edit forms sit at the foot of the page. Expected:
each page shows its main action in sight (a primary button), and each empty state says why it is empty and what to
run or open.

# Findings
* `templates/home.html` says `no_domain_generated` with no action; `no_run.html` is the page before a first run.
* The review form (WEB-81) and the edit form (WEB-93) are on the pages of a domain, a feature and a use case; the
  debt page (`debt_list.html`) links to the sections but offers no action on them.
* `issues/done/web_ui_guided_entry.md` added the "where to start" links of the home page.

# Approach
1. List per page its main action and its empty states (a table in the issue at the plan), validated with the user
   before coding (method, not a list of items to judge).
2. Primary button near the title of the section pages (review, edit), actions on the rows of the debt list,
   empty states with the command to run (`retrodoc generate`) or the link to open.

# Specs
* `specs/web-ui.md`: adds the actions and the empty states that become requirements.

# Milestone
A web interface that invites to act

# Resources
* `templates/` (all pages), `macros.html`; `crates/retrodoc-web/src/pages.rs`, `debt.rs`
* `specs/web-ui.md` (WEB-9 no run, WEB-79 to WEB-93 review and edit)
