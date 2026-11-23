---
priority: low
depends_on: [references_resolved_by_codegraph]
why: 'a measured gain in slice precision that no milestone needs yet; the port is worth it only if its mix with the lexical fallback holds'
---
# Resolve the references of the parsed files through imports and receivers

# Goal
A call resolved by its bare name brings method-name noise into the slices
(`issues/done/parsed_references_for_ranking_and_slices.md`); codegraph's resolution, which knows imports, parent
classes and receivers, removes it where it links but sees nothing for a fifth of the entry-point files
(`issues/references_resolved_by_codegraph.md`, `docs/findings.md` §13). Expected: the same precision from
RetroDoc's own `structure.rs`, with the lexical follow for what it does not resolve, and slices a reader judges
better than today's, with no external tool.

# Findings
* 2026-11-23, linkding, Lobsters, delivery_router, no LLM: lexical vs codegraph slices common / lexical only /
  codegraph only 35 / 133 / 49 and 241 / 488 / 302; mean slice 3.8 and 5.7 files against 7.6 and 7.7. Where
  codegraph links, the five slices read hold no method-name noise and add the models, queries and parent classes.
* codegraph links nothing for 24 of 119 entry-point files: Django commands calling each other by `call_command`
  strings, `.rake` tasks and extensionless scripts (not parsed as Ruby), two small views, two base classes.
* The exports of the measure (`edges.tsv` per run) are kept in `references_resolved_by_codegraph-7ad832a-1/` of the
  measure folder: a reference to compare a port against, file pair by file pair.

# Approach
Measure first, with no LLM, before touching the pipeline.

1. Measure the mix: a `Follow` of `parsed_slices` that takes codegraph's edges first and the lexical follow for the
   files they leave empty, on the same runs; read a sample. Stop if it is not better than the lexical slice.
2. Port, language by language, starting with Python and Ruby (the benchmark repositories): imports resolved to
   files (relative and package paths), a call on `self`, a class constant or a parent class resolved within the
   class's hierarchy, a call on an unknown receiver left unresolved (no bare-name guess); compare its edges with the
   kept `edges.tsv`.
3. Use it in `CodeIndex::slice` behind the same bounds, the lexical follow as fallback (ADR 0036), then consider
   `ranking.rs`.

# Resources
* `issues/references_resolved_by_codegraph.md`, `issues/done/parsed_references_for_ranking_and_slices.md`
* `docs/findings.md` §13, ADR `0022`, ADR `0027`, ADR `0036`
* `crates/retrodoc-ingest/src/structure.rs`, `crates/retrodoc-pipeline/src/slices.rs`, `ranking.rs`
* the `parsed_slices` example of `retrodoc-bench` (`--codegraph`)
* codegraph's resolution, MIT: `https://github.com/colbymchenry/codegraph` (`src/`, TypeScript)

# Hints
* Per language the work differs: a per-language resolver must stay behind its grammar feature, and a language with
  no resolver keeps today's path.
* Do not copy codegraph's framework routing: per-framework adapters were rejected (ADR 0036).
