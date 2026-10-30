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

# Tracking

Plan: validated

Done: every deliverable committed; follow-ups in `issues/done/stack_tables_nested_layouts.md`.

## Understanding
Make the walker's built-in tables (source extensions, test heuristic) and the manifest reader of the product brief
cover the reference set of `specs/language-agnostic.md`, so these stacks need no roles call to be classified and
briefed. Out of scope: grammars (LNG-8), framework fixtures (LNG-9), both have their own issue.

## Plan
1. [x] Walker tables: add `mjs`, `cjs`, `mts`, `cts`, `kts`, `cc` (a `*.gradle.kts` build script stays out of the
   source), and the `Test`/`Tests` suffix with a capital after a word; verified by a `retrodoc scan` test on a repo
   with one file per extension and one test per layout (source/test counts), plus unit cases (`Contest.java`,
   `build.gradle.kts`); makes true LNG-5, LNG-6. ~150 lines.
2. [x] Manifests: read `build.gradle`, `build.gradle.kts` (dependency coordinates `group:artifact` from the
   `implementation`-like calls), `*.csproj` at the root under any name (`PackageReference Include`, name from
   `AssemblyName` else the file stem), `Package.swift` (package name, `.package(url:` dependencies), `build.sbt`
   (`name :=`, `"org" %% "artifact"` dependencies), by string scanning, no new crate; verified by a
   `retrodoc brief --signals` test (13 manifests counted at the root) plus one unit test per format and an
   unparseable one; makes true LNG-7. ~350 lines.
3. [x] High review fixes: build scripts (`*.gradle.kts`, `Package.swift`) stay out of the source even when the
   roles call names their extension; the capital `Test` suffix only for the languages whose convention it is;
   Gradle reads dependency configurations only (no JDBC URL, no `*Test*` configuration); sbt drops test-scoped
   dependencies; `.csproj` `Include` in any attribute order, `$(...)` names ignored; verified by unit cases;
   amends the LNG-5 and LNG-6 scenarios.

## Decisions
- Mode: autonomous
- Base commit: 9dbfd7c8cd849e7b160af90d6c786b18df65534f
- Gradle gives no project name (it lives in `settings.gradle`, not a manifest of LNG-7): dependencies only (user, ok).

## Deliverable 1
- done: walker knows `mjs`, `cjs`, `mts`, `cts`, `kts`, `cc`, keeps `*.gradle.kts` out of the source, and reads
  `FooTest`/`FooTests` as tests.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clean |
| specs | LNG-5, LNG-6 traced through `retrodoc scan` |
| docs | `docs/crates/core-ingest.md` |
| code review | low, run |

- red: LNG-5, LNG-6: `scan` counted `source: 19, other: 9` for 25 sources, and `source: 3, test: 5` for 8 tests.
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 2
- done: the brief reads `build.gradle`, `build.gradle.kts`, `Package.swift`, `build.sbt` and every root `*.csproj`.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clean |
| specs | LNG-7 traced through `retrodoc brief --signals` |
| docs | `docs/crates/core-ingest.md` |
| code review | low, run |

- red: LNG-7: `brief --signals` counted `Manifest 8` for 13 manifests.
- Code review: no findings
- not verified: none
- decisions needed: none

## Deliverable 3 (high review of `9dbfd7c..925f52c`)
- done: 7 of the 10 findings fixed; the 3 others (Gradle version catalogs, nested `.csproj`, .NET test-project
  directories) plus `project/*.scala` and `BloodTest.java` go to `issues/done/stack_tables_nested_layouts.md`.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clean |
| specs | LNG-5, LNG-6 scenarios amended |
| docs | `docs/crates/core-ingest.md` |
| code review | low, run |

- red: LNG-5, LNG-6: `build.gradle.kts` promoted to source by `kts` (4 promoted, 3 expected), `SpeedTest.tsx`
  classified Test; Gradle listed `androidx.test:runner, jdbc:postgresql`; sbt kept `scalatest, mockito-core`;
  csproj missed `Npgsql` and named `$(MSBuildProjectName)`.
- Code review: 2 findings, both fixed: `IncludeAssets` matched before `Include` in a `PackageReference`, and an sbt
  scope `% "it,test"` cut at its comma; cases added to the unit tests. Second low review: no findings.
- not verified: none
- decisions needed: none
