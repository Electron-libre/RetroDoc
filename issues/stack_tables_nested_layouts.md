# Read the nested build layouts of the JVM and .NET stacks

# Goal
The built-in stack tables now cover the reference set of `specs/language-agnostic.md` at the root of a repository
and by file name. The high review of `stack_tables_cover_the_reference_set` (2026-10-30) found the common layouts
they still miss: the brief gets no dependency or no manifest at all, or test scaffolding is read as source. Make
these layouts give the same signals as the flat ones.

# Findings (code read on 2026-10-30, high review of `stack_tables_cover_the_reference_set`, not seen on a run yet)
* A Gradle build on a version catalog (`implementation(libs.spring.boot.web)`, the default of new Gradle
  projects) has no quoted coordinates: `gradle_manifest` finds no dependency and the manifest signal is dropped.
  The catalog is `gradle/libs.versions.toml`.
* `*.csproj` is read only at the root; the usual .NET layout has a root `.sln` and the projects under
  `src/<Project>/<Project>.csproj`: no manifest signal.
* .NET test projects (`Shop.Tests/`, `Shop.UnitTests/`) and Gradle source sets (`src/integrationTest/`) are not in
  `TEST_DIRS`: only their `*Tests.cs` files are tests, their fakes and builders are source.
* sbt build code under `project/*.scala` (`Dependencies.scala`) is classified source.
* A domain class named `BloodTest.java` or `LabTest.php` is taken for a test by the capital `Test` suffix (LNG-6):
  an accepted trade-off, to revisit only if a run shows such a loss.

# Approach
1. Read `gradle/libs.versions.toml` (`[libraries]` `module` or `group`/`name`) as a manifest.
2. Read the `.csproj` files the root `.sln` lists, or those one or two levels down, bounded.
3. Take a directory component ending in `.Tests`/`Tests` (and `UnitTests`, `IntegrationTests`), and the Gradle
   source sets ending in `Test` under `src/`, for test directories.
4. Keep the root `project/` directory of an sbt build (a `build.sbt` next to it) out of the source.

# Specs
* `specs/language-agnostic.md`: LNG-5, LNG-6, LNG-7 (scenarios to add).

# Milestone
Language and framework agnostic

# Resources
* `crates/retrodoc-ingest/src/walker.rs`, `crates/retrodoc-ingest/src/signals/manifest.rs`
* `docs/crates/core-ingest.md`
* `docs/adr/0036-language-and-framework-agnostic-by-design.md`

# Hints
* A file newly classified as test leaves the file budget and the fingerprints: a saved run re-reads the passes.

# Tracking

Plan: validated

## Understanding
Make the nested JVM and .NET layouts give the signals of the flat ones: Gradle version catalog and the `.csproj`
of a solution as manifests, .NET test projects and Gradle test source sets as tests, the sbt `project/` out of the
source. Out of scope: the `BloodTest.java` trade-off (a non-goal of the spec), telling catalog test libraries apart.

## Plan
1. [x] Walker layouts: a directory ending in `Tests` and a Gradle source set `src/<name>Test/` or `src/testFixtures/` are test
   directories; the root `project/` of an sbt build is kept out of the source, in `walk_repo` and
   `promote_to_source`; verified by `stack_tables.rs` through `scan` and walker unit tests; makes true LNG-13, LNG-14
   (~150 lines).
2. [x] Gradle version catalog: `gradle/libs.versions.toml` read as a manifest (`module`, `group`/`name`, string
   notation); verified by `stack_tables.rs` through `brief --signals` and `manifest.rs` unit tests; makes true LNG-11
   (~120 lines).
3. [x] Nested `.csproj`: the projects the root `.sln` lists, else one or two levels down, bounded, test projects
   and paths outside the repo left out; verified likewise; makes true LNG-12, sets the spec `accepted` (~200 lines).

## Decisions
- Mode: autonomous
- Catalog: every library of `[libraries]`, test ones included (answer 1 a).
- `Tests` directory rule: every language (answer 2 a).
- `src/testFixtures/` is a test directory (answer 3 a).
- Base commit: ca76e57564bb06779392fff1e58d56dde7b07ee7

## Deliverable 1: walker layouts
- done: `*Tests` directories, `src/<name>Test/` and `src/testFixtures/` are test directories; the root `project/` of
  an sbt build is `Other` in `walk_repo` and never promoted.
- checks: `just check` ok; docs `docs/crates/core-ingest.md`.
- red: LNG-13, LNG-14: `Files: 9 (source: 7, test: 1, ...)` and `Files: 4 (source: 2, test: 0, markdown: 0, other: 2)`.
- Code review: no findings.
- not verified: none. decisions needed: none.

## Deliverable 2: Gradle version catalog
- done: `gradle/libs.versions.toml` is a manifest: the `group:artifact` of each `[libraries]` entry (string,
  `module`, `group`/`name`), in key order.
- checks: `just check` ok; docs `docs/crates/core-ingest.md`.
- red: LNG-11: `left: None, right: Some(1)` (no manifest read from the catalog).
- Code review: no findings.
- not verified: none. decisions needed: none.

## Deliverable 3: nested `.csproj`
- done: the `.csproj` of the root solutions (`.sln` and `.slnx`, paths inside the repo only), else one or two levels
  down (hidden, `bin`, `obj`, `node_modules` skipped), test projects left out, at most 20; the spec is `accepted`.
- checks: `just check` ok; docs `docs/crates/core-ingest.md`.
- red: LNG-12: `left: None, right: Some(1)` and `left: None, right: Some(2)`.
- Code review: no findings.
- not verified: none. decisions needed: none.
