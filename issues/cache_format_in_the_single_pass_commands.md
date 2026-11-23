---
priority: low
depends_on: [format_version_of_the_saved_artifacts]
why: 'only bites at the first bump of the cache format, none is planned yet'
---
# Keep the cache format stamp true in the single-pass commands and through `--force`

# Goal
`generate` records the format of `.retrodoc/cache/` and the readers of the docs refuse another (`specs/cache-format.md`),
but two paths can leave a cache stamped with a format some of its files are not in, which the readers then trust.
Expected: no command writes an artifact under a stamp of another format, and no artifact of an older format is kept
under the current stamp.

# Findings
* 2026-11-19 (`high` review of `format_version_of_the_saved_artifacts`): the single-pass commands (`roles`, `glossary`,
  `entry-points`, `actors`, `business-read`, `business-files`, `contexts`, `surface`, `brief`) write artifacts into the
  cache without checking or recording `format.json`; an older binary running one of them on a cache a later version
  stamped writes files of the older format under the later stamp.
* 2026-11-19 (same review): `generate --force` on a cache of an older format keeps the files `--force` does not clear
  (`roles.yaml`, `roles-unmatched.yaml`, `signal-sources.yaml`, `product.yaml`, `business-files.yaml`, `contexts.yaml`,
  `usage.json`), reads them leniently, then stamps the cache with the current version.

# Approach
1. The single-pass commands check the format as `generate` does (`cache_format::check`, an older cache refused without
   `--force`, a later one always) and record it before their first write.
2. For `--force` on an older cache, candidates to choose at the plan:
   1. the format bump states which kept files it changes, and `--force` clears those too;
   2. `--force` on an older cache clears every artifact (the hand-edited ones included, said in the message);
   3. an upgrade in place of the kept files at the bump that changes them.

# Specs
* `specs/cache-format.md` (CFV): new requirements for the single-pass commands and the kept files.

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0050, `specs/cache-format.md`, `crates/retrodoc-pipeline/src/cache_format.rs`.
* `Artifact::cleared_by_force` in `crates/retrodoc-pipeline/src/artifact.rs`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
No command leaves a file of one format under the stamp of another: the single-pass commands check the format (an
older cache refused even with `--force`, which only redoes one artifact) and those that write record it;
`generate --force` on an older cache moves the whole cache aside to `.retrodoc/cache-format-<found>/`. Out of scope:
the benchmark replays, an upgrade in place. `surface` writes nothing: it only checks.

## Plan
1. [x] Single-pass commands: one helper of `workspace.rs` checks the format (CFV-11) and records it before the first
   write (CFV-12), called by the nine commands; verified by tests through the binary in `tests/cache_format.rs`
   (format 0 and 99 caches, a fresh repo); makes true CFV-11, CFV-12. (autonomous)
2. [ ] `generate --force` on an older cache moves it to `.retrodoc/cache-format-<found>/` (a function of
   `retrodoc_core::cache::format`, one call in `generate`, none under `--estimate`) and prints the directory and the
   hand-editable files moved; verified by a test through the binary (format 0 cache with an edited `roles.yaml`) and
   one with `--estimate`; makes true CFV-13; ADR 0052 accepted.

## Decisions
- Mode: autonomous
- Base commit: fa5fefcb036c2112720a286fa13981b3be828d0a
- `--force` on an older cache: the whole cache moved to `.retrodoc/cache-format-<found>/` (question 1, option a).

## Summary of deliverable 1
- done: the nine single-pass commands refuse a cache of another format (older included, `--force` or not); the eight
  that write record the format at their start (`Workspace::open_to_write`), `surface` only checks.
- red: CFV-11, CFV-12: `["roles", "--force"] did not fail` on a format 0 cache; `format.json` missing after `roles`
  on a fresh repo.
- Code review: no findings
- not verified: none. decisions needed: none.
