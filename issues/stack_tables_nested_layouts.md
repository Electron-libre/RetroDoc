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
