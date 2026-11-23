---
priority: medium
why: 'the actors page is one of the two pages under "Read" and cannot be read on an application of real size; no milestone needs it'
---
# Rework the actors page so that each actor reads as a whole

# Goal
The actors page (`/actors`) is a matrix: a row per actor, a column per feature (WEB-57 to WEB-60). On an application
of real size it has dozens of columns, most of its cells empty, and its rows are as tall as their longest cell
anywhere in the row, so the reader scrolls in two directions to find what one actor does. The expected outcome is a
page where a reader sees at a glance who the actors are, and for one actor what it starts and takes part in,
without a horizontal scroll.

# Findings
* 2026-11-23, the docs of the linkding benchmark baseline (DeepSeek), at 1440 px: 43 feature columns plus the
  permission rules column; 8 actors; the filled cells per actor are 10, 24, 3, 9, 7, 1, 1 and 43 of 44. Six of the
  eight rows fill less than a quarter of their cells.
* The row "Administrator" shows empty for its first three columns and is about 300 px tall, because its use cases
  sit in columns out of sight; the reader cannot tell an empty row from a row whose cells are off screen.
* The generic actor "System" takes part in 43 of the 44 columns: it says little and fills the widest row.
* The permission rules column is the last of 44, out of sight on every screen.
* Since the wide tables scroll in their own frame with the actor column kept in sight (`.wide` in
  `crates/retrodoc-web/assets/style.css`), the page no longer scrolls as a whole, but the matrix stays mostly empty.

# Approach
Propose, then decide at `issue-workflow` step 1; the options can be combined.
1. One section per actor instead of a matrix: its name, how many use cases it starts and takes part in, then its use
   cases grouped by domain and feature (those it starts first or marked), then its permission rules. An index of the
   actors at the top, each with its counts, links to its section.
2. Keep a compact overview: actors × domains (5 to 15 columns rather than 40+), each cell a count linking to the
   section of the actor, the detail moving to option 1.
3. Set apart the generic system actors ("System", "Calling program") under their own heading or folded, since they
   take part in almost everything.
4. Give each actor its own page (`/actors/<slug>`) and keep `/actors` as the index; the tree and the search could
   then link to an actor.

# Specs
* `specs/web-ui.md`: amends WEB-57 to WEB-60 (the matrix) and WEB-66 (the count of the "Start here" line) if the
  shape of the page changes; a page per actor (option 4) adds requirements.

# Resources
* `crates/retrodoc-web/templates/actors.html`, `crates/retrodoc-web/src/actors.rs`.
* `specs/web-ui.md` WEB-57 to WEB-60; `docs/dev/web-ui.md` (screenshots before and after, components).
* `issues/done/web_ui_rules_and_actors.md`: why the page was a matrix.

# Hints
* Shoot the page before and after with `just screenshots` on a run of real size (the linkding baseline in the runs
  directory has 43 features); the test repos of the crate are too small to show the problem.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
Replace the actors × features matrix by an index of the actors (people then systems, each with what it starts and
takes part in) and a section per actor (its use cases by domain and feature, those it starts first, then its
permission rules); the map of a domain links an actor to its section. Out of scope: a page per actor, an overview by
domain, a special treatment of the generic system actors (non-goals of `specs/web-actors.md`).

## Plan
1. [x] Actors page by actor: `specs/web-actors.md` (WAC, WEB-57 to WEB-60 moved there), `actors.rs` and
   `actors.html` rewritten as index + sections, the slug of WAC-7, texts in English and French, the `.actor` styles;
   verified by `tests/actors_page.rs` rewritten on the WAC ids and shots before/after on the linkding baseline
   (`just screenshots`); makes true WAC-1, WAC-2, WAC-3, WAC-4, WAC-6, WAC-7, WAC-8. (autonomous)
2. [x] Map links to the section: each actor of `/map/<domain>` links to `/actors#<id of WAC-7>`, the slug shared with
   the actors page; verified by a test of `tests/map_page.rs`; makes true WAC-5, sets `specs/web-actors.md` and
   `specs/web-ui.md` accepted. (autonomous)
3. [x] Fixes of the high review and docs of the whole issue: actors of use cases out of the run left out, use cases
   read once per feature, slug of any script, a domain the run lacks named without a link, a stronger empty state,
   a stale doc comment; verified by `tests/actors_page.rs`; amends WAC-7, adds scenarios to WAC-3 and WAC-6. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: c57b897d0d6a33ee03ea378fa1839eb087deb286
- Answers of step 2 (2026-11-23): 1a a spec of its own; 2a no page per actor; 3a "System" stays with the system actors, not folded.
- Approach: options 1 and 3 of the issue (the system actors already have their own heading, after the people);
  options 2 and 4 left out (non-goals).
- The actors page gets a spec of its own (`specs/web-actors.md`, WAC) rather than growing `specs/web-ui.md`, which
  `check-specs` already flags as over its bounds; the move of the rest of `web-ui.md` under `### Behaviour` /
  `### Contract` is not done here (120 requirements untouched).
- `crates/retrodoc-web/src/texts.rs` (1,509 lines, over the limit) gets the few strings of the page beside the others;
  its split is not part of this issue.

## Deliverable 1: actors page by actor
- done: `/actors` is an index of the actors (people then systems, each with "starts n · takes part in m") then a
  section per actor (`#actor-<slug>` / `#system-<slug>`) listing its use cases by domain and feature, those it starts
  first and marked, then its permission rules; no table; `specs/web-actors.md` (WAC) written, WEB-57 to WEB-60 moved
  there; `docs/crates/render-mcp-cli.md` and `docs/dev/web-ui.md` updated. Shots on the linkding baseline:
  `~/retrodoc-runs/screenshots/before-actors-c57b897` and `after-actors-by-actor` (no horizontal scroll at 1440 nor 500 px).
- red: WAC-1, WAC-2, WAC-3, WAC-4, WAC-6, WAC-7, WAC-8: `no entry #actor-clerk`, `no section actor-clerk` in the
  matrix page, and `<table` found on the page.
- Code review: 1 finding
  - fixed: a system actor named `Actors` took the id `system-actors` of the list of the index (duplicate id, its
    link jumping to the list); the ids of the lists now count as taken, WAC-7 amended with its scenario and a test
    (written with the fix, not run red first).
- not verified: none beyond the eye check of the shots.

## Deliverable 2: map links to the section
- done: each actor of `/map/<domain>` links to `/actors#<its section>` (`ActorsPage::human_href`, the actors page now
  built before the maps in `Site::new`), the top of the page as a fallback; happy paths WAC-H1 and WAC-H2 traced;
  `specs/web-actors.md` and `specs/web-ui.md` accepted; `docs/crates/render-mcp-cli.md` updated.
- red: WAC-5: `href="/actors#actor-clerk"` not found in the list of the map (it linked `/actors`).
- Code review: no findings
- not verified: none.

## Deliverable 3: fixes of the high review
- done: the actors come from the use cases of the features of the run only (no 0/0 actor of a feature the run
  lacks); each feature's use cases are read once (`domains_in_order`), not once per actor; the slug keeps letters of
  any script (WAC-7 amended); a domain the run does not list is named without a link; the empty-state test also
  refuses the index; the doc comment of `start_links` says "a section per actor".
- red: WAC-3, WAC-6, WAC-7: `<h3>shipping</h3>` not found (it was a link to a missing page), `Auditor` listed,
  `no section actor-équipe-support` (it was `actor-quipe-support`).
- high review findings: 5 (1 important, 4 nits), all fixed as above.
- Code review: no findings (low review of this deliverable)
- not verified: none.
