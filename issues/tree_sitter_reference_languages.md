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

# Tracking

Plan: validated

## Understanding
Compile in the 9 missing grammars of the reference set, each behind its feature, so the structure gives definitions,
lines and calls for every reference language (LNG-8). Out of scope: languages outside the reference set, a framework
reader. The prompt of the roles pass names the parsed extensions: adding one changes its fingerprint (ADR 0037).

## Measure (2026-10-30, scratch crate on `tree-sitter` 0.27)
| Grammar | Crate | `TAGS_QUERY` as is | Fix needed | `parser.c` |
|---|---|---|---|---|
| Java | `tree-sitter-java` 0.23.5 | ok: class, interface, method, call | none | 2.5 MB |
| Kotlin | `tree-sitter-kotlin-ng` 1.1.0 | none shipped (`tree-sitter-kotlin` 0.3 needs `tree-sitter` < 0.23) | own query | 22 MB |
| Scala | `tree-sitter-scala` 0.26.3 | `tags.scm` in the crate, not exported | query kept in the repo | 27 MB |
| C# | `tree-sitter-c-sharp` 0.23.5 | rejected: stray `@module` capture | drop that pattern | 29 MB |
| PHP | `tree-sitter-php` 0.25.1 | class, function ok; no call found | extra call pattern | n/a |
| Go | `tree-sitter-go` 0.25.0 | ok: type, method, call | none | 1.5 MB |
| C | `tree-sitter-c` 0.24.2 | function spans its declarator line only; no call | extra patterns | 3.7 MB |
| C++ | `tree-sitter-cpp` 0.23.4 | same as C | extra patterns | 17 MB |
| Swift | `tree-sitter-swift` 0.7.4 | class, method, protocol ok; no call | extra call pattern | 23 MB |

All depend on `tree-sitter-language` 0.1, so they load in `tree-sitter` 0.27. Build: each grammar's C build takes
0.3 to 2.7 s (release, 9 together: 11 s wall). Size: the 9 add ~22 MB to a release binary (today `retrodoc` is
31 MB).

## Plan
1. [x] JVM and .NET: features `java`, `kotlin`, `scala`, `csharp`; Kotlin query written, Scala query kept in the
   repo, C# query filtered; Java, Kotlin and Scala one family in `same_language`; default features decided; docs
   (`docs/crates/core-ingest.md`, ADR 0022 note); verified by one structure test per language and a family test;
   makes true part of LNG-8. ~400 lines.
2. [x] PHP and Go: features `php`, `go`, PHP call pattern; verified by one test per language; part of LNG-8. ~200 lines.
3. [ ] C, C++ and Swift: features `c`, `cpp`, `swift`, patterns for whole function bodies and calls; C and C++ one
   family; verified by one test per language and a family test; completes LNG-8, spec `accepted`. ~350 lines.

## Decisions
- Mode: autonomous
- Base commit: 60d14f2ebe321e65cdf308ed74dc0e8ef0bfad49
- Every grammar feature on by default (+~22 MB release binary): LNG-8 holds for the default build.
- `.h` is read with the C++ grammar, which reads most C headers too.
- Kotlin: `tree-sitter-kotlin-ng` with a query written in the repo.

## Deliverable 1 summary
done: Java, Kotlin, Scala and C# parsed (features on by default); Java, Kotlin and Scala one family; a `grammar`
feature set by every language replaces the list of features in the `cfg_attr`s; ADRs left as they are (records),
`docs/crates/core-ingest.md` updated.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean, also with one language feature alone |
| specs | LNG-8 traced (Java, Kotlin, Scala, C#) |
| docs | `docs/crates/core-ingest.md` |
| code review | see below |

red: LNG-8: `called Option::unwrap() on a None value` (no grammar for `.java`, `.kt`, `.scala`, `.cs`); family test:
`same_language("a.kt", "b.java")` was false.
Code review: no findings

not verified: a run on a real JVM or .NET repository (no smoke repo of those stacks).
decisions needed: none

## Deliverable 2 summary
done: PHP and Go parsed (features on by default), PHP query completed with enums and plain function calls; the
slices test of a file the build cannot parse moved from Go to Zig.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clippy pedantic clean, also with `php` or `go` alone |
| specs | LNG-8 traced (PHP, Go) |
| docs | `docs/crates/core-ingest.md` |
| code review | see below |

red: LNG-8: `called Option::unwrap() on a None value` (no grammar for `.php`, `.go`).
Code review: no findings

not verified: a run on a real PHP or Go repository.
decisions needed: none
