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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Each page puts its main action in sight and each empty state says why and what to run or open. Out of scope: new
actions (the review and edit forms stay as they are), the empty states that already say why and what to do. Means:
links to the existing forms, `?edit=1` unfolds the edit form on the server (no script).

### Main action and empty states per page (method: one primary action per page, an empty state = why + command or link)

| page | main action | empty states | change |
|---|---|---|---|
| home | debt page, with its count; map when no debt (WEB-116) | no domain: why + `retrodoc scan`, `retrodoc generate` (WEB-117) | new |
| domain, feature, use case | "Review" (primary) → `#review`, "Edit" → `?edit=1#edit-form` (WEB-113, WEB-114) | feature with no use case: says why (WEB-16) | new actions |
| debt | "Review", "Edit" on the rows of sections (WEB-115) | none in run, no score, no docs, none matches: already why + command or reset link | rows |
| map | its domains | no domain: as home (WEB-117) | empty state |
| no run (WEB-9), debt before a run (DBT-14), ask (no docs), audit, history (fewer than two runs) | — | already name `retrodoc generate` | none |
| rules, glossary, actors | filters / lists | "none found in the code of the last run": why given, nothing to run | none |
| search | — | not documented + other words (WEB-42) | none |
| code, files, collected doc, domain map | links | already say why | none |

## Plan
1. [x] Section actions: buttons under the heading of the domain, feature and use case pages, `?edit=1` unfolds the
   edit form; verified by page tests; makes true WEB-113, WEB-114. (autonomous)
2. [x] Debt row actions: "Review" and "Edit" on the items of sections; verified by debt page tests; makes true WEB-115. (autonomous)
3. [x] Home main action and empty states of home and map; verified by home and map tests; sets the spec `accepted`;
   makes true WEB-116, WEB-117. (autonomous)
4. [x] Fixes of the `high` review of the issue: the debt actions named by their section, `forms.js` drops
   `?edit=1`, no main action with no domain, `StartLink` and `DebtRow` without derived fields; verified by the
   tests of WEB-115, WEB-116. (autonomous)
5. [x] Quiet actions on the debt rows: secondary "Review", quiet "Edit"; verified by the test of WEB-115
   and the screenshots. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 1e6cf2ce9404bfe348097d7d884a89ca9caa7be5
- Answers: 1 a (home button to the debt page with its count, map when none), 2 a (`?edit=1` unfolds on the server), 3 a (row actions on sections only).

## Deliverable 1: section actions
- done: "Review" and "Edit" buttons under the heading of the domain, feature and use case pages; `?edit=1` unfolds
  the edit form.
- red: WEB-113, WEB-114: no `<nav class="section-actions"` after `</h1>`; `?edit=1` left `<details>` folded.
- Code review: no findings
- not verified: the look in a browser (screenshots) is checked at the end of the issue.

## Deliverable 2: debt row actions
- done: the items of a domain, a feature or a use case of the run offer "Review" and "Edit"; rules, folders and
  sections the run lacks offer none.
- red: WEB-115: `no actions in` the first item of `/debt`.
- Code review: no findings
- not verified: the look in a browser, checked at the end of the issue.

## Deliverable 3: home main action and empty states
- done: the home page leads to the debt page with its count, to the map with none; a run with no domain says why
  and to run `retrodoc scan` then `retrodoc generate`, on the home and map pages; the spec back to `accepted`.
- red: WEB-116, WEB-117: no `<p class="main-action">` on the home page; the empty state said only "The last run
  generated no domain."
- Code review: no findings
- not verified: the look in a browser, checked at the end of the issue.

## Deliverable 4: fixes of the `high` review
- done: the actions of a debt item labelled with its section; `forms.js` drops `?edit=1` like `?saved=`; no main
  action on the home page with no domain (WEB-116 amended), a run with no debt tested on its own; `StartLink::label`
  and `DebtRow::section` removed for the fields they derived from.
- red: WEB-115: `named by its section`; WEB-116: `class="main-action"` shown with no domain.
- Code review (`high`, whole issue): 8 findings: 6 fixed here (a11y label, same-URL Edit, `?edit=1` kept in the
  address bar, dead-end map button, redundant `StartLink::label`, redundant `DebtRow::section`); 1 partly (the
  `/debt` literal compared once in `start_links` instead of twice, still matched by `MainAction::of`, which the
  tests of WEB-116 cover); not fixed: the round trip of "Edit" to the server, the means the user chose (answer 2 a).
- Code review: no findings (`low`, this deliverable)
- not verified: the `forms.js` change runs only in a browser.

## Deliverable 5: quiet actions on the debt rows
- done: on the rows of the debt list, "Review" is secondary and "Edit" quiet, the gold kept for the pages of a
  section (screenshots: 46 gold buttons on the debt page of linkding); decided by the user.
- red: none (a change of look, the test of WEB-115 follows the markup).
- Code review: no findings
- not verified: nothing.
