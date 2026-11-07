---
priority: low
why: 'no reader breaks today, but a run saved by an older version is read silently; cheap ground for the docs bundle'
---
# Write a format version with the saved artifacts and check it at load

# Goal
The readers of the generated docs read the YAML artifacts of `.retrodoc/cache/` with no way to know which version of
RetroDoc wrote them; a field renamed or removed reads as missing (defaults) rather than as an older format.
Expected: the artifacts read by the readers carry a format version, an older one is refused or upgraded with a clear
message, and a change of format is a deliberate, tested step.

# Findings
* 2026-11-07: no format or schema version constant in `crates/`; the edits store already refuses a file "of a later
  format" (SED-13 of `specs/section-edit.md`).
* 2026-11-07: `GeneratedDocs::load` falls back to defaults on an artifact it cannot read (`unwrap_or_default` on the
  use cases, domain map, glossary, rules).

# Approach
1. List the artifacts the readers load (see `one_loader_of_the_generated_docs`) and how each reacts to an unknown
   or missing field today.
2. Candidates, to choose at the plan:
   1. one manifest file per run with the format version, written by `generate`, checked once at load;
   2. a version field in each artifact read outside the pipeline;
   3. on an older format: a refusal naming the command to run again (`render` or `generate`) against an upgrade in
      place.
3. The version must not enter the fingerprints, so a rerun stays a no-op.

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0003, ADR 0005, ADR 0037, ADR 0043.
* `specs/section-edit.md` (SED-13), `specs/rerun.md`.
