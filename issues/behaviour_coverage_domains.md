# Build domains and features from entry points and entities, and cover behaviours instead of files

# Goal

Make the cost of a run grow with the business surface of the application instead of its file count, and
stop forcing technical files into business domains. Implement ADR 0020: every entry point belongs to one
use case of one feature, domains are clustered from the entities and the entry points, features group
entry points, the LLM repo map becomes optional, and files reached by no behaviour are reported as
technical support.

# Findings

* First full `generate` on the Rails test repository (2026-10-02, 325 source files, local model): about
  2h20, with 201 file calls and 89 directory calls for the repo map; the ~2,300-file repository is still
  out of reach (`PLAN.md` §7.2).
* The features pass reads only the file summaries of its domain (`features/mod.rs`), not its entry points
  nor the glossary.
* The same file is read by the LLM in the glossary, entry points, repo map, use cases and confidence
  passes.
* The confidence pass is about a third of the calls (`PLAN.md` §7.2).

# Approach

1. Measure first on `delivery_router` and a sparse checkout of the Rails test repository: share of files
   reached by an entry point slice or holding an entity, calls and time per pass (`usage.json`).
2. Domains from the surface: cluster entities and entry points grouped by resource, framed by the product
   brief (`issues/done/product_brief.md`); attach files deterministically from slices and entity homes.
3. Features from entry points: one call per domain over its entry points (verb, resource, outputs) and
   entities; use cases keep their per-entry-point slices.
4. Coverage of entry points by construction (place, then uncategorized), reported; files reached by
   nothing are listed in the debt report.
5. Repo map: make the LLM summaries opt-in; use a deterministic map (symbols, references, ranking) for
   the structure, see `issues/tree_sitter_structure.md`.
6. One read per business file: one call returns the entities, entry points, outputs and rules of a file.
7. Confidence: deterministic checks first (named identifiers at the cited lines, named entities in the
   slice), the LLM verdict on a sample and on the doubtful steps.
8. Rekey the incremental re-run on entry points and entities.
9. Compare with `issues/quality_benchmark.md`.

# Resources

* ADR `0020`, and the ADRs it affects: `0004`, `0005`, `0008`, `0011`
* `crates/retrodoc-pipeline/src/domains/`, `features/`, `use_cases/`, `slices.rs`, `ranking.rs`,
  `repo_map/`, `confidence/`, `fingerprints.rs`
* `crates/retrodoc-cli/src/commands/generate/mod.rs` (pass order)

# Hints

* This is the largest change of the redesign: ship it in steps that each leave `generate` working, and
  keep the old path behind a setting until the benchmark says the new one is better.
* A library's public API is its entry points (ADR 0008); `delivery_router` is a library.
* A missed entry point is a missed use case: the report must make the gap visible.
