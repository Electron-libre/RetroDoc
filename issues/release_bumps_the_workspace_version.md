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
