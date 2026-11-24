---
priority: medium
why: 'the actors page is one of the two pages under "Read" and is still too long to consult after its rework by actor; no milestone needs it'
---
# Make the actors page short enough to consult

# Goal
Since `web_ui_actors_page_by_actor`, `/actors` is an index of the actors then a section per actor listing every use
case it starts or takes part in, by domain and feature (`specs/web-actors.md`). The matrix is gone, but the page is
still one long list: on an application of real size the reader scrolls through hundreds of lines to reach one actor
or to see what it does. The user judges it still far too complex and long (2026-11-24). The expected outcome is a page
a reader consults rather than reads through: who the actors are and, for the actor they pick, what it does, each in
about one screen, the full detail one click away.

# Findings
* 2026-11-24, `/actors` served on the docs of the linkding benchmark baseline (DeepSeek, run `dafea18`): 8 actors,
  about 92 KB of HTML, 497 list items; the index alone gives each actor's "starts n · takes part in m", then the
  8 sections follow in full, none folded.
* Items per section: Administrator 30, Bookmark owner 80, Guest visitor 7, API integration 29, Calling program 18,
  Internet Archive 2, OIDC identity provider 2, System 156.
* The generic actor "System" takes part in 113 use cases: its section is about a third of the page and says little
  (the non-goal "the generic system actors are not recognised" of `specs/web-actors.md`).
* Each use case is a full line under its feature, under its domain heading; an actor that starts 56 use cases
  (Bookmark owner) fills several screens on its own.
* The 1440 px shot of `after-actors-by-actor` (in the runs directory) is cut at 4,000 px, far before the end of the page.
* The non-goals of `specs/web-actors.md` exclude today a page per actor, an overview by domain, a special treatment
  of the generic system actors and the folding of the use cases: any option below amends them.

# Approach
Measure first: shoot `/actors` before (`just screenshots`) on the linkding baseline and note the full page height.
Then propose, decide at `issue-workflow` step 1; the options can be combined.
1. Fold each section (`<details>`) so that the page shows the index and the actors' headings with their counts; the
   section opened by the anchor the map links to (`#actor-<slug>`) is open.
2. Summarise each actor by domain and feature with counts ("Bookmark Management: 3 features, 12 use cases"), the
   list of the use cases folded or left to the feature page.
3. A page per actor (`/actors/<slug>`), `/actors` kept as the index; the map and the search link to it.
4. Set apart the generic system actors ("System", "Calling program"): folded by default, or reduced to their counts.
5. An overview actors × domains with counts in the cells (8 rows, a dozen columns), each cell linking to the detail.

# Specs
* `specs/web-actors.md`: amends WAC-2, WAC-3, WAC-6 (the shape of the sections) and the non-goals the chosen options
  lift; a page per actor (option 3) adds requirements and amends WAC-5 (the link from the map).

# Resources
* `crates/retrodoc-web/templates/actors.html`, `crates/retrodoc-web/src/actors.rs`, `.actor` styles of
  `crates/retrodoc-web/assets/style.css`.
* `specs/web-actors.md`; `docs/dev/web-ui.md` (screenshots before and after, components).
* `issues/done/web_ui_actors_page_by_actor.md`: the previous rework and the decisions that set the non-goals.

# Hints
* The test repos of the crate are too small to show the problem: judge on the linkding baseline (43 features) with
  `just screenshots`; the shots stop at 4,000 px, so measure the full height another way.
* A folded section must still open on the anchor the map links to, or the link lands on a closed heading.
