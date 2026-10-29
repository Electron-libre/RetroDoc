# Give the joint reading the routing-file rule of the entry points prompt

# Goal
The joint reading (`business_read`, on by default since ADR 0028) asks for the entities and the entry points in
one prompt, written by hand from the two single-purpose prompts. It lacks one rule of the entry points prompt: a
routing file gives one entry per resource or namespace, not one per route. A routing file read jointly can then
come back with one entry per route. Bring the rule back, and keep the prompts from drifting again.

# Findings (exploration of 2026-10-29, at aee4cc9; read in the code, not measured)
* `crates/retrodoc-pipeline/src/business_read/mod.rs`: the comment above `BUSINESS_READ_SYSTEM_PROMPT` says the
  wording is that of `glossary` and `entry_points`, "keep them close"; the routing-file sentence of
  `ENTRY_POINTS_SYSTEM_PROMPT` (`entry_points/mod.rs`) is missing.
* The effect depends on whether a routing file is among the business files: those outside the list (entry point
  role) are read by the entry points pass alone, with its own prompt. Not measured.

# Approach
1. Measure first: on the frozen inputs of `lobsters` and `linkding`, is a routing file read by the joint reading,
   and how many entries does it give? (Ledger first: reuse a saved run if one has the business read.)
2. If it is: add the rule to the joint prompt, or build the joint prompt from parts shared with the two prompts so
   a change of one reaches the other.
3. Measure the entry points again.

# Resources
* ADR 0028; `crates/retrodoc-pipeline/src/business_read/`, `entry_points/`, `glossary/`
* The measures ledger (see `CLAUDE.md`), skill `quality-benchmark`

# Hints
* The per-file hash of the reading passes covers the content, the brief and the model; check whether it covers
  the prompt, else a saved run keeps the old entries until the file changes.
