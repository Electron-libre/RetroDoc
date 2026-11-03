---
priority: high
why: 'every binary released since v0.1.0 reports a wrong version; small and needed before the next release'
---
# Make the released binary report its version

# Goal
`retrodoc --version` prints `0.1.0` while the last tag is `v0.6.0`: `just release` (a Cargo-agnostic Cardinal
command) writes the CHANGELOG section and the tag but leaves `[workspace.package] version` untouched. A user, a bug
report or a cached artifact cannot tell which release produced it. Expected: the workspace version is the released
one, and a release cannot be tagged with a version the workspace does not carry.

# Findings (audit of 2026-11-03)
* `Cargo.toml`: `[workspace.package] version = "0.1.0"`, every crate has `version.workspace = true`.
* `git show v0.6.0:Cargo.toml` has `0.1.0` too; tags `v0.1.0` to `v0.6.0` exist.
* `retrodoc --version` prints `retrodoc 0.1.0`.
* `just release` is `cardinal run release` (`cardinal.just`), a managed file: it cannot be edited here.

# Approach
1. Set `[workspace.package] version` to the last released version (`0.6.0`) and refresh `Cargo.lock`.
2. Keep it from drifting again, candidates (can be combined):
   (a) a test or `just check` step that fails when the latest `v*` tag differs from the workspace version on the
   tagged commit or after it;
   (b) ask Cardinal upstream for a release hook that bumps a project-chosen version file before the tag (outside this
   repository; an issue there);
   (c) a documented manual step in `docs/dev/commands.md` before `just release <version> --tag`.

# Milestone
Release and code hygiene

# Resources
* `Cargo.toml`, `cardinal.just`, `CHANGELOG.md`, `docs/dev/commands.md`.

# Hints
* Never edit the Cardinal-managed files (`just check-cardinal` fails); project values live in `.claude/cardinal.toml`.
* The version may feed fingerprints or artifacts: check that bumping it does not invalidate the caches of saved runs
  (`grep -rn CARGO_PKG_VERSION crates`).

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
The workspace version must be the released one (`0.6.0` now) and a release must not go out with a version the
workspace does not carry. Out of scope: changing Cardinal's `release` here (managed file). Means: instead of comparing
with the latest `v*` tag (CI checks out without tags), a test compares `retrodoc --version` with the newest
`## [x.y.z]` section of `CHANGELOG.md`, which `just release <version>` writes before the tag: `just check` fails
until the version is bumped, so the release commit and its tag carry it.

## Plan
1. [x] Bump and guard: `[workspace.package] version = "0.6.0"`, `Cargo.lock` refreshed, a test in
   `crates/retrodoc-cli/tests/version.rs` that runs the binary and fails when its version differs from the newest
   CHANGELOG section, the release step documented in `docs/dev/commands.md`; verified by the test (red at `0.1.0`,
   green at `0.6.0`) and `just check`; no spec (release hygiene, not a capability). (autonomous)

## Decisions
- Mode: autonomous
- Base commit: b5e73ed8e41f4049acdaa33eaeb986c5c0a4b6fe
- Cardinal upstream hook (option b): not asked; the test is the guard (answer 1 a).
- No cache impact: no crate reads `CARGO_PKG_VERSION` (`grep -rn CARGO_PKG_VERSION crates` is empty); only clap's
  `#[command(version)]` uses the version.

## Deliverable 1 summary
- done: workspace version `0.6.0`, test `tests/version.rs` (binary vs newest CHANGELOG section), release note in
  `docs/dev/commands.md`, row "Release process" in `.claude/update-docs-map.md`.
- red: none (no spec): `left: "retrodoc 0.1.0"  right: "retrodoc 0.6.0"`
- Code review: no findings
- not verified: nothing
- decisions needed: none
