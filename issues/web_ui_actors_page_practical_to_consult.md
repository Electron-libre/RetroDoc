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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-24

## Understanding
`/actors` becomes the index only (who the actors are, one screen: 8 lines on linkding) and each actor gets its page
`/actors/<id>`: its domains and features with counts, the use cases folded under each feature. Options 3 and 2 of the
issue, the folding of option 1 applied per feature; options 4 and 5 left out. Measured before: 10,374 px high at
1440 px (13 screens), 11,846 px at 500 px; on the actor page Bookmark owner shows 5 domains and 24 features, System
5 and 43 (shots: `~/retrodoc-runs/screenshots/before-actors-practical-1f904893`).

## Plan
1. [x] Page per actor: `specs/web-actors.md` amended (WAC-1 to WAC-5, WAC-7, WAC-8, WAC-9, WAC-10, non-goals);
   `actors.rs` gives the index and the page of an actor (`ActorsPage::actor(id)`), `actors.html` the index only, a
   new `actor.html`, the route `/actors/{id}` (404 of WAC-9), the map links to the actor page, the texts in English
   and French beside the others in `texts.rs`, the `.actor` styles; verified by `tests/actors_page.rs` and
   `tests/map_page.rs` on the WAC ids, the height of `/actors` and of the page of the largest actors measured on the
   linkding baseline before/after (`just screenshots` plus a chromedriver measure); makes true WAC-1 to WAC-10. (autonomous)
2. [x] Fixes of the high review: the feature link out of the `<summary>` (the counts fold), a grid instead of CSS
   columns, the singular kind of the actor, the breadcrumb, the href of an actor kept on the model, `actor_pages.rs`
   and WEB-73 in the docs; verified by `tests/actors_page.rs` and the shots. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 1f9048933c14c382c3faee1b68ed5b431c6277ac
- Answers of step 2 (2026-11-24): 1a index plus a page per actor; 2a path `/actors/<WAC-7 id>`; 3a the search does not list the actors; 4a "System" is a line of the index like the others.
- `texts.rs` (1,509 lines) and `view.rs` (1,027) are over the file limit: the few strings go beside the others in
  `texts.rs` (its split is not this issue), the lookup of an actor lives in `actors.rs`, not `view.rs`.
- `pages.rs` went over 800 lines with the handler of the actor page: the two handlers of the actors pages moved to
  `actor_pages.rs`. `texts.rs` stays over the limit (one string added), as decided above.

## Deliverable 1: page per actor
- done: `/actors` is the index only, each actor linking to `/actors/<id>` (its entry carries the id, so a former
  `/actors#actor-<slug>` lands on it); the page of an actor gives its domains with their counts, its features in
  columns with what it starts and takes part in, the use cases folded under each feature, then its permission rules;
  unknown id 404; the map links to the actor page; `just screenshots` also shoots an actor page. Measured on the
  linkding baseline (chromedriver, `scrollHeight`): `/actors` 10,374 → 757 px at 1440 px (one screen), 11,846 → 930 px
  at 500 px; Administrator 831 px, Bookmark owner 1,290 px, System 1,822 px at 1440 px (no horizontal scroll); these three
  were measured with the stylesheet of the base cached by the browser, see deliverable 2 for the right figures. Shots:
  `~/retrodoc-runs/screenshots/before-actors-practical-1f904893`, `after-actors-practical`.

| check | result |
|---|---|
| tests | pass (`tests/actors_page.rs` 19, `tests/map_page.rs`) |
| lint | pass (clippy pedantic, -D warnings) |
| specs | `specs/web-actors.md` accepted, WAC-1 to WAC-10 traced |
| docs | `docs/crates/render-mcp-cli.md`, `docs/dev/web-ui.md`, a doc comment of `view.rs` |
| code review | see below |

- red: WAC-1, WAC-2, WAC-3, WAC-4, WAC-6, WAC-7, WAC-8, WAC-9, WAC-10: `/actors/actor-clerk` answered 404 (`left: 404,
  right: 200`), `no entry actor-clerk` in the index, the use case links found on `/actors`; WAC-5 was not run red
  apart (the actors tests failed first in the same `cargo test`).
- Code review: no findings
- not verified: the opening of a fold by keyboard and screen reader (native `<details>`, not tested); Firefox and
  Safari not shot.
- decisions needed: none.

## Deliverable 2: fixes of the high review
- done: the six nits of the `high` review: the feature is a link followed by a `<details>` whose summary is its
  counts (a click on the name no longer leaves the page by surprise, no link nested in a summary); a grid of columns
  (opening a fold moves no sibling); the kind is singular (`Person`, `System`); the breadcrumb `Home › Actors › <name>`
  replaces the ad hoc back link; `ActorSection::href` set once, used by the index and the map; `actor_pages.rs` named
  in `docs/crates/render-mcp-cli.md` and `docs/ARCHITECTURE.md`; WEB-73 says its actors link to their page (WAC-5).
  Measured again on the linkding baseline with a fresh browser (bottom of the content / window 757 px): `/actors`
  398 px at 1440 px, 862 px at 500 px; Administrator 719 px, Bookmark owner 1,000 px, System 1,371 px at 1440 px;
  2,066 and 3,096 px at 500 px; no horizontal scroll.

| check | result |
|---|---|
| tests | pass (`tests/actors_page.rs` 19, `tests/map_page.rs` 21) |
| lint | pass |
| specs | pass; WEB-73 reworded, still traced |
| docs | `docs/dev/web-ui.md`, `docs/crates/render-mcp-cli.md`, `docs/ARCHITECTURE.md` |
| code review | see below |

- red: WAC-2: the kind was asserted `Person` after the code changed (not run red first); WAC-10: the assertion that
  the summary holds no link was added after the fix (not run red first).
- Code review: no findings
- not verified: Firefox and Safari not shot; keyboard and screen reader use of the folds not tested.
- decisions needed: none.
