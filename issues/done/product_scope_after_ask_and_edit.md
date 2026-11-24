---
priority: medium
why: 'the v1 scope of PRODUCT.md is the reference for what is in or out, and it now contradicts what was released'
---
# Bring the v1 scope of PRODUCT.md in line with what was released

# Goal
`PRODUCT.md` still lists features as "Later" or out of v1 after they were released, so its scope no longer tells
what v1 is. Expected: the capabilities table, the "Scope of v1" and the "Ask the documentation (vision)" section say
what is released, what is still later, and where v1 ends.

# Findings
* 2026-11-07: the capabilities table puts "Ask the documentation from RetroDoc itself (`ask` / `chat`, the ask page
  of `ui --ask`)" under "Later" only; `ask` and `chat` were released in v0.11.0, the ask page in v0.12.0
  (`ROADMAP-done.md`).
* 2026-11-07: the web interface row puts "edit the docs" under "Later" and "Scope of v1" puts "editing the docs in
  the web interface" out of v1; the milestone "Edit the docs" was released in v0.10.0.
* 2026-11-07: the vision section says "The first form is the MCP server", without the `ask` agent that followed.

# Approach
1. Move the released capabilities to the v1 column and drop them from "Out of v1".
2. Ask the user whether v1 ends now (the next milestones being validation) or still takes features, and write the
   answer as the boundary of "Scope of v1".
3. Check `README.md` and `docs/ARCHITECTURE.md` against the same scope (`skill:update-docs`).

# Resources
* `PRODUCT.md`, `ROADMAP-done.md`, `CHANGELOG.md`.
* Skill `update-docs`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-24

## Understanding
Make `PRODUCT.md` say what v1 holds as released: ask/chat and the ask page, editing the docs, and the web interface's
debt, audit, run history and review no longer "planned"; the vision section names the `ask` agent after the MCP
server. Out of scope: any code change. No spec (docs only).

## Plan
1. [x] Scope of v1 in line with the releases: capabilities table (ask row and web row moved to v1, "Later" keeps
   hosting for a team), a user journey "Ask the docs", the vision section naming `ask` / `chat` / `ui --ask` as the
   second form, "Out of v1" without editing, the boundary of v1 written from the user's answer; `README.md` status
   and `docs/ARCHITECTURE.md` checked (`skill:update-docs`); verified by `cardinal run project-check` and a reread
   against `ROADMAP-done.md`; makes true the Goal of the issue. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 051cf2b153673e04fbfb51f61359a557a8920778
- Boundary of v1 (answer 1 a): v1 is complete in features; the open milestones ("A local model, measured", "Read by
  someone taking an application over") validate it; any new feature comes after v1.

## Deliverable 1 summary
- done: `PRODUCT.md` capabilities table, principle "read-only", user journeys 6 and 7, vision (second form: `ask`),
  boundary of v1 and "Out of v1" aligned with v0.7.0 to v0.12.0.
- checks: project-check, deliverable-size, code review low.
- red: none (docs only, no test).
- update-docs: `README.md` status and `docs/ARCHITECTURE.md` already true, ADRs historical: n/a.
- Code review: no findings
- not verified: nothing.
2. [x] Fixes of the `high` review: principle "read-only" (config, run history in the cache, `chat` and `ui --ask`
   call the LLM), journey 7 and the web row (what an edit covers, who flags an outdated change); verified by
   project-check and a reread against `specs/section-edit.md` and `specs/section-review.md`. (autonomous)

## Deliverable 2 summary
- done: the 5 nits of the `high` review fixed in `PRODUCT.md`.
- checks: project-check, code review low.
- red: none (docs only, no test).
- Code review: no findings
- not verified: nothing.

Done: PRODUCT.md says what v1 holds and where it ends; nothing left.
