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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
`scan` (reads `roles.yaml`) and `benchmark` (reads the domains and features of the last run) are readers of the
cache: both call `check_format` first and refuse a cache of another format (CFV-14). Out of scope: `benchmark-table`
(reads JSON figures) and the frozen inputs of the replays (already a non-goal).

## Plan
1. [x] Check the format in `scan` and `benchmark`: `check_format` at the start of both, before any LLM call or
   artifact read; verified by a CLI test in `crates/retrodoc-cli/tests/cache_format.rs` (older and later format,
   `scan`, `benchmark`, `benchmark --judge` with no LLM call, `scan` on a repo with no cache); makes true CFV-14. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 31750ac8a5a7b0eee261ff3ec1adb17327f5158e
- 1 a: `benchmark` refuses too (a later bump asks `generate --force` before scoring an older run).
- 2 a: `scan` refuses, no warn-and-skip variant.

## Summary of deliverable 1
- done: `scan` and `benchmark` call `check_format` first; CFV-14 added and traced, spec back to `accepted`.

| check | result |
|---|---|
| tests | pass (`cache_format`: 14) |
| lint | pass |
| specs | pass |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | see below |

- red: CFV-14: `["scan"] did not fail`
- Code review: no findings
- not verified: none
- decisions needed: none

## Summary of deliverable 2
- done: the 5 findings of the `high` review (all nits or pre-existing) fixed.

| check | result |
|---|---|
| tests | pass (`cache_format`: 15, `promoted_sources`: 6) |
| lint | pass |
| specs | pass |
| docs | `docs/ARCHITECTURE.md`, `cache/format.rs`, `specs/cache-format.md` |
| code review | see below |

- red: CFV-14: `unknown field \`a_later_key\`, expected \`name\`` (scan read the config before the format)
- Code review: no findings
- High review of the issue: 5 findings, all fixed: the format checked before the config in `scan`; the fixtures of
  `promoted_sources.rs` record the format; `ARCHITECTURE.md` and `format.rs` name the two commands; the non-goal of
  scoring a kept run of an older format (decision 1 a); `history` dropped from the Intent (pre-existing).
- not verified: none
- decisions needed: none
