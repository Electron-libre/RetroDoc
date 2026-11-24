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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-24

## Understanding
A section a unit writes again (use case, feature, domain) takes back the saved slug of the section of its unit with
the same entry points (resources for a domain), one-to-one, never merging two; measured on the saved run of §15: the
2 renamed use cases out of 17 had exactly the entry points of a lost saved one, no feature nor domain renamed.
Out of scope: listing the saved slugs in the prompt (HCR-8 unchanged, non-goal of `specs/rerun.md`).

## Plan
1. [x] Saved slugs kept by the use cases pass: the spec amendment, a module `saved_slugs.rs` (the one-to-one match on
   entry points), used by `grounded_use_cases`; verified by a binary test in `crates/retrodoc-cli/tests/human_corrections.rs`
   (a use case renamed by the model keeps its slug and its edit) and unit tests of the module; makes true RER-13,
   RER-14, RER-15. (autonomous)
2. [ ] Saved slugs kept by the features pass: the features of a slice matched against the saved features of the
   domain in that slice, a slug held by another slice or domain left out; new tests in
   `features/from_surface/slug_tests.rs` (`from_surface.rs` and its `tests.rs` are over the file size limit: a few
   call lines only there); makes true RER-13, RER-14, RER-15 for the features.
3. [ ] Saved slugs kept by the domains call: matched on their resources, `uncategorized` never matched; tests in
   `domains/tests.rs`; makes true RER-13, RER-16; the spec set `accepted`.

## Decisions
- Mode: autonomous
- Base commit: 95079e3922c37231aac348188791014c872879a2
- Saved slugs not listed in the prompt: the match is mechanical only (question 1 a).

## Deliverable 1: summary
done: the use cases pass gives back the saved slug of a use case written again with the same entry points (`saved_slugs.rs`).

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (`specs/rerun.md` draft, RER-13 to RER-15 traced, RER-16 left to deliverable 3) |
| docs | `docs/crates/pipeline.md` (Saved slugs), `docs/ARCHITECTURE.md` §5 |
| code review | see below |

red: RER-H1, RER-13: `use-cases.yaml` held `slug: browse-orders` instead of `list-orders` after the use case was asked again.
Code review: 1 findings
- `held_elsewhere` always false for the use cases, a slug of another feature could be given back: not fixed, a use
  case slug is unique within its feature only (`grounded_use_cases` filters the taken slugs by feature, a use case is
  named `<feature>/<use case>`).

not verified: no paid run of the default smoke repository (the measure of the saved run shows the 2 renames had equal entry points).
decisions needed: none
