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
