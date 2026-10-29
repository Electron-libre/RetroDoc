# Make the built-in stack tables cover the reference languages

# Goal
`specs/language-agnostic.md` names a reference set of the most frequent languages (ADR 0036). The walker's source
extensions, its test-path heuristic and the manifests the product brief reads were written from the smoke and
benchmark repositories and miss some of them. Make each table cover the reference set, so that a Java, C#, Kotlin,
PHP or Swift repository is classified and briefed without relying on the roles call.

# Findings (code read on 2026-10-29 at 94a83b9)
* `FileKind::from_path` (`walker.rs`) lacks `mjs`, `cjs`, `mts`, `cts`, `kts` and `cc` (LNG-5).
* `is_test_path` knows the `test`/`tests`/`spec` directories and the `_test`, `_spec`, `.test`, `.spec` suffixes
  and `test_` prefix: `OrderTest.java`, `OrderTests.cs`, `OrderTest.php` outside a test directory are source files
  (LNG-6). Maven's `src/test/` is already caught by the `test` directory.
* `MANIFESTS` (`signals/manifest.rs`) reads 8 fixed names: no `build.gradle`, `build.gradle.kts`, `*.csproj`
  (a name per project, not a fixed one), `Package.swift`, `build.sbt` (LNG-7).

# Approach
1. Extend the extension list and the test heuristic; the `Test`/`Tests` suffix needs a capital after a word
   (`Contest.java` stays source).
2. Read the missing manifests: name and dependencies where the format allows it cheaply (Gradle and SBT are code:
   the name and the dependency coordinates by regex, or the file as is, bounded); `*.csproj` found by extension at
   the root.
3. Trace LNG-5, LNG-6, LNG-7 with tests through the binary where cheap (`retrodoc scan`, `brief --signals`).

# Specs
* `specs/language-agnostic.md`: LNG-5, LNG-6, LNG-7.

# Resources
* `docs/adr/0036-language-and-framework-agnostic-by-design.md`
* `crates/retrodoc-ingest/src/walker.rs`, `crates/retrodoc-ingest/src/signals/manifest.rs`
* `docs/crates/core-ingest.md`

# Hints
* A file newly classified as source enters the file budget and the fingerprints: a saved run re-reads it.
