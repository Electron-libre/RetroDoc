# Parse every reference language with tree-sitter

# Goal
The structure (definitions, references, signature views, cuts of long files) parses Ruby, Rust, JavaScript,
TypeScript and Python. `specs/language-agnostic.md` (LNG-8, ADR 0036) asks for every language of the reference set:
add Java, Kotlin, C#, PHP, Go, C, C++, Swift and Scala, each behind its cargo feature.

# Findings (code read on 2026-10-29 at 94a83b9)
* `structure.rs`: `Lang::of` maps extensions to 5 grammars, each behind a feature of `retrodoc-ingest`.
* The other languages take the generic path: LLM boundary regex, then blank lines (ADR 0013); no definitions nor
  references for the ranking and the slices.
* Unknown: whether each grammar crate ships a `tags.scm` query usable as is, its version compatibility with the
  workspace's `tree-sitter`, and the build time and binary size each adds.

# Approach
1. Measure first: for each of the 9 grammars, the crate, its `tags.scm`, the build time and binary size added.
2. Add the grammars in two or three deliverables (JVM and .NET: Java, Kotlin, C#, Scala; then PHP, Go; then C, C++,
   Swift), each with the tests of its definitions and references.
3. Decide with the measure whether all features stay on by default.

# Specs
* `specs/language-agnostic.md`: LNG-8.

# Resources
* `docs/adr/0022-tree-sitter-for-code-structure.md`, `docs/adr/0027-parse-to-cut-long-files-and-opt-in-short-bodies.md`,
  `docs/adr/0036-language-and-framework-agnostic-by-design.md`
* `crates/retrodoc-ingest/src/structure.rs`, `issues/done/tree_sitter_structure.md`

# Hints
* `same_language` groups the languages that call each other (JavaScript and TypeScript): Java, Kotlin and Scala
  are one such family on the JVM.
* A newly parsed language changes the cuts of its long files: saved readings of those files are redone.
