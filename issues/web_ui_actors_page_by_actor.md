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
