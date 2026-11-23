---
priority: low
depends_on: [cache_format_in_the_single_pass_commands]
why: 'only bites at the first bump of the cache format, none is planned yet'
---
# Check the cache format in `scan` and `benchmark`

# Goal
Every command that reads the generated docs or a single pass refuses a cache of another format (`specs/cache-format.md`),
but `scan` and `benchmark` still read artifacts of the cache with no check: a cache of another format is read
leniently, as empty or missing fields, with no error. Expected: both refuse it as the other readers do, or the spec
states why they need not.

# Findings
* 2026-11-23 (`high` review of `cache_format_in_the_single_pass_commands`): `scan` loads `roles.yaml` through
  `RoleRules::load` to count the promoted languages, and `benchmark` reads the domains and features of the last run;
  neither calls `cache_format::check`, so a cache a later RetroDoc wrote is read leniently.

# Approach
1. Decide, per command, whether it reads the cache as a reader of the docs (check it) or as a measure on frozen runs
   (a non-goal, as the benchmark replays already are).
2. Call `check_format` (`crates/retrodoc-cli/src/commands/workspace.rs`) at the start of those that check, and amend
   `specs/cache-format.md` (a new requirement or a non-goal).

# Specs
* `specs/cache-format.md` (CFV).

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0050, ADR 0052, `specs/cache-format.md`.
* `crates/retrodoc-cli/src/commands/scan.rs`, `crates/retrodoc-cli/src/commands/benchmark.rs`.
