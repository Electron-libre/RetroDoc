---
rank: 400
why: 'low (high review of `stack_tables_nested_layouts`, 2026-10-30): small, last issue of milestone `Language and framework agnostic`; layouts no run has shown yet'
---
# Read the singular, BDD and Android variant test layouts as tests

# Goal
The walker now reads the .NET test projects ending in `Tests` and the Gradle test source sets as tests (LNG-13).
The high review of `stack_tables_nested_layouts` (2026-10-30) found the layouts it still misses: their fakes,
builders and step definitions are read as source, and the `.csproj` of such a project passes the test filter of the
manifest reader, so its test libraries reach the brief as product dependencies. Make them tests.

# Findings (code read on 2026-10-30, high review of `stack_tables_nested_layouts`, not seen on a run yet)
* A .NET test project named in the singular (`Shop.Api.Test/`, `Shop.UnitTest/`) or a SpecFlow/BDD one
  (`Shop.Specs/`) is not matched: only the plural `Tests` suffix is.
* The Android variant test source sets (`src/testDebug/`, `src/androidTestDebug/`) are not matched: a source set
  must end in `Test` or be `test`.
* A directory ending in `Tests` is a test directory in every language (decision of `stack_tables_nested_layouts`);
  a domain directory such as `LabTests/` is lost with it: an accepted trade-off, to revisit only if a run shows it.

# Approach
1. Take a .NET project directory ending in `.Test`, `UnitTest`, `IntegrationTest` or `.Specs` for a test directory
   when it holds a `.csproj` or `.cs` files, to keep `Contest/` and `specs/` of other stacks as they are.
2. Take a Gradle source set starting with `test` or `androidTest` (`testDebug`, `androidTestRelease`), with a JVM
   language directory below, for a test directory.

# Specs
* `specs/language-agnostic.md`: LNG-12, LNG-13 (scenarios to add).

# Milestone
Language and framework agnostic

# Resources
* `crates/retrodoc-ingest/src/walker.rs`, `crates/retrodoc-ingest/src/signals/manifest.rs`
* `docs/crates/core-ingest.md`

# Hints
* A file newly classified as test leaves the file budget and the fingerprints: a saved run re-reads the passes.
