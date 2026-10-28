# Use parsed references to rank files and follow slices

# Goal

Replace the lexical guesses of `ranking.rs` (the files mentioning a file's name) and `slices.rs` (referenced
identifiers followed by text) with the references a parse gives, keeping the lexical path where the parser
sees nothing. Expected: slices that hold the code a behaviour really runs, and a ranking that does not
reward a name that only appears in a comment. Worth doing only if a measure shows a difference.

# Findings

* `retrodoc_ingest::structure::parse` already returns, per file, its definitions and the names it calls
  (ADR 0027); nothing in the pipeline reads the references yet.
* The `tags.scm` queries of the five grammars only give calls: no constants, types, imports or class
  inheritance. Ruby metaprogramming and Rails conventions (callbacks, `has_many`, routes to controller
  actions by name) hide most links from a parser.
* `ranking.rs` is about 290 lines and `slices.rs` about 250.
* Step 4 of `issues/done/tree_sitter_structure.md` showed how noisy a single local-model run is; a
  comparison of slices must be made without an LLM, on the slices themselves.

# Approach

1. Measure first, with no LLM: an example that, for the entry points of a saved run, prints the slice built
   today and the one the parsed calls would build (files in common, files only in one), on the benchmark
   repositories and the Rails test repository.
2. If they differ in a way a reader judges better, resolve a called name to the definitions of the repo
   (by name, then by file proximity) and use it in `CodeIndex::slice`, behind the same bounds (depth 2, 8
   files); keep the lexical follow for the names the parse does not resolve.
3. Feed `ranking.rs` with the resolved references instead of file-name mentions, keeping the mentions as a
   fallback; compare the files kept under `--max-files` before and after.

# Resources

* ADR `0022`, ADR `0027`
* `crates/retrodoc-ingest/src/structure.rs`, `crates/retrodoc-pipeline/src/slices.rs`, `ranking.rs`
* `crates/retrodoc-pipeline/examples/behaviour_baseline.rs` (the slice is already cut there)

# Hints

* A call by name is ambiguous (`save`, `call`): resolve only names with one or few definitions in the repo.
* Keep the cargo features of the grammars independent: a build without a language must fall back to the
  lexical path.

# Tracking

Plan: validated

## Understanding
Measure, with no LLM, whether slices built from parsed calls (resolved to the repo's definitions) differ from
today's lexical slices in a way a reader judges better; only then switch `slices.rs`, then `ranking.rs`, to the
resolved references with the lexical path as fallback. Out of scope: richer `tags.scm` queries, Rails conventions.

## Plan
1. [x] Measure: a `DefinitionIndex` in `retrodoc-pipeline` (name -> defining files, from `structure::parse`;
   only names with at most a few definitions, closest by path first) and an example `parsed_slices` that, per
   entry-point file of a saved run, prints the lexical slice and the parsed one (in common, only lexical, only
   parsed) plus totals; run it on `rules-ld-judged/`, `rules-dr-judged/` and the saved Rails test run; verified
   by unit tests of the index and of the example's comparison; results recorded here and in the ledger.
   **Stop after it**: the user judges whether the parsed slices are better (decides deliverables 2 and 3).
2. [-] (dropped, see Decisions) Slices: spec `specs/code-slices.md` (written at this deliverable, once the measure says go), then
   `CodeIndex::slice` follows resolved calls first, the lexical follow for unresolved names, same bounds
   (depth 2, 8 files); a build without a grammar keeps today's slices; verified by tests traced to the spec.
3. [-] (dropped, see Decisions) Ranking: `ranking.rs` counts resolved references instead of file-name mentions, mentions kept as
   fallback; verified by tests traced to the spec and by the files kept under `--max-files` before and after
   on the same saved runs.
4. [x] Fixes of the `high` review: a call counts as resolved by its definitions of the same language
   whatever the slice already holds; TypeScript and TSX one language without the JavaScript feature;
   fixtures with generic names; doc of the example; verified by unit tests.

## Decisions
- Mode: autonomous (the chain stops after deliverable 1 by design)
- Base commit: f0dfa48940c9c77aeff8bca73430c88a286ff944
- Deliverables 2 and 3 dropped (user, 2026-10-28): the parsed slices differ from the lexical ones by as
  much noise as they remove, and resolving only to non-method definitions makes them almost the lexical ones; the
  `parsed_slices` example and `Follow` stay as the measuring tool.
- Measure compares three slices per entry-point file: lexical (today), parsed + lexical fallback (what would ship), resolved calls only (what the parser sees).

## Results of deliverable 1

`parsed_slices` on four saved runs (linkding, delivery_router, lobsters, a large Rails repository), slices of
depth 2 and 8 files, from each entry-point file alone; figures after the fixes of deliverable 4; columns: mean slice size, then files common / lexical only / other only, summed.

| run | entry-point files | lexical | mixed | parsed alone | lexical vs mixed | mixed = lexical | lexical vs parsed |
|---|---|---|---|---|---|---|---|
| linkding | 22 | 7.6 | 7.6 | 5.9 | 140 / 28 / 28 | 5 | 34 / 134 / 95 |
| delivery_router | 2 | 8.0 | 8.0 | 5.5 | 16 / 0 / 0 | 2 | 11 / 5 / 0 |
| lobsters | 95 | 7.7 | 7.7 | 7.2 | 499 / 230 / 237 | 9 | 268 / 461 / 413 |
| large Rails repository | 245 | 8.0 | 8.0 | 8.0 | 1176 / 784 / 784 | 19 | 369 / 1591 / 1590 |

* Ruby's `tags.scm` gives the constants a file reads as references too (`[(identifier) (constant)]`), not only
  calls; Python and JavaScript give calls only.
* A first run resolved calls across languages (Ruby calls to vendored JavaScript): definitions are now taken
  from files of the same language only (`structure::same_language`); the figures above are after it.
* The difference comes from method names: called on a receiver whose type the parser does not know
  (ActiveRecord attributes, `up`/`down`), they resolve to whatever file defines a method of that name (a console
  initializer, an old migration). A trial that resolves only to non-method definitions (classes, modules,
  functions) makes the mixed slice almost the lexical one: lobsters 636 / 93 / 97, 29 files equal; the Rails
  repository 1873 / 87 / 87, 178 of 245 files equal.
* Read on lobsters: the mixed slice drops lexical noise (`app/javascript/user.js` for `user`, `config/routes.rb`)
  and finds included concerns (`story_finder.rb`), but brings in as much method-name noise (`lib/time_series.rb`,
  an old migration, `config/initializers/telebugs.rb`). The parsed slice alone loses most lexical links in
  Python (no constants in its query).
* Artifacts: `~/retrodoc-runs/parsed_references_for_ranking_and_slices-f0dfa48-1/` (`detail.md` before the
  same-language fix, `detail-same-language.md`, `detail-no-methods.md`, `detail-after-review.md`).

## Summary of deliverable 1

done: `CodeIndex::with_definitions`, `slice_with` and `Follow` (lexical, parsed, parsed then lexical),
`structure::same_language`, and the `parsed_slices` example; the passes still use the lexical follow.

| check | result |
|---|---|
| tests | `just check` passes |
| lint | clippy pedantic clean |
| specs | n/a (no pipeline behaviour changed) |
| docs | `commands.md`, `core-ingest.md`, `pipeline.md`, module doc of `slices.rs` |
| code review | `low`, no findings |

red: none (no spec); before the code the tests failed to compile (`no method named with_definitions`,
`cannot find type Follow`).

Code review: no findings

not verified: whether a reader prefers the mixed slices; the use cases a slice change would give (LLM).

decisions needed: go or not for deliverables 2 and 3 (see Results).

## Summary of deliverable 4

done: fixes of the `high` review on `f0dfa48..HEAD`; figures of the measure re-run (unchanged conclusion);
ADR 0027 records the result.

| check | result |
|---|---|
| tests | `just check` passes; `retrodoc-ingest` also with `--no-default-features --features typescript` |
| lint | clippy pedantic clean |
| specs | n/a |
| docs | ADR 0027, doc of `parsed_slices` |
| code review | `low` on the fixes, no findings |

red: none (no spec); before the fix `a_resolved_call_never_falls_back_on_the_file_name` would have found
`app/models/refresh.rb` by its name.

Code review: no findings

High review: 10 findings; fixed: TypeScript family and its test without the JavaScript feature (2), resolution
independent of the slice and of `from` (2), useless clone, fixtures named after a confidential repository, doc of
the per-file slice; not fixed: double parse and copied `sources_of`/constants (measuring tool only, pattern of the
other examples), method names resolved (the measured finding itself, reason of the drop).

not verified: nothing beyond deliverable 1.

decisions needed: none.
