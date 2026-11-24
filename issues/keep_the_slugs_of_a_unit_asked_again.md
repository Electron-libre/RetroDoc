---
priority: medium
depends_on: [human_edits_and_reviews_guide_the_next_run]
why: 'a review marked wrong rewrites a whole unit, and the new slugs of its other sections orphan their edits and reviews'
---
# Keep the slugs of the sections a unit asked again writes again

# Goal
A unit asked again (a feature's use cases call, a domain's features call, the domains call) writes all its sections
again, and the model gives some of them new slugs. An edit or a review names its section by slug (SED-1, REV-1): a
renamed section loses them (SED-24), although only one section of the unit was corrected. Expected: a section the
unit writes again keeps the slug it had when it is the same section.

# Findings
* 2026-11-24, default smoke repository, deepseek-chat (`docs/findings.md` §15): a use case marked wrong asked its
  feature again; the corrected use case kept its slug (shown to the model, HCR-8), but 2 of its 3 sibling use cases
  came back under new slugs (`compute-…` to `estimate-…`) for the same entry points.

# Approach
1. Measure on saved runs how often a unit asked again renames a section whose entry points are unchanged.
2. Candidates, to combine: show the model the slugs of all the saved sections of the unit and ask to keep them;
   match a new section to a saved one by its entry points mechanically and give it back the saved slug.

# Specs
* `specs/human-corrections.md` (HCR-8 widened to every saved section of the unit), `specs/rerun.md`.

# Milestone
Read by someone taking an application over

# Resources
* `crates/retrodoc-pipeline/src/use_cases/grounding.rs`, `crates/retrodoc-pipeline/src/features/from_surface.rs`,
  `crates/retrodoc-pipeline/src/corrections.rs`, `specs/section-edit.md` (SED-24).

# Hints
* The features pass already merges features of the same slug or name across slices: a mechanical match must not
  merge two sections the model split on purpose.
