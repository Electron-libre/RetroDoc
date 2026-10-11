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
