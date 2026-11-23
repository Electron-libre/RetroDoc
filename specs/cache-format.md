---
id: CFV
status: accepted
adrs: [0003, 0005, 0037, 0043, 0050, 0052]
---

# Format version of the saved artifacts

## Intent

The commands that read the generated docs back (`render`, `report`, `ui`, `mcp`, `ask`, `chat`, `search`, `scan`,
`benchmark`)
must know that the artifacts of `.retrodoc/cache/` were written in the format they read, so a cache written by
another version of RetroDoc is refused with the command to run rather than read as empty fields.

## Requirements

### Behaviour

- **CFV-1** (deterministic) `generate` must record the format version of the artifacts it writes in the cache.
- **CFV-2** (deterministic) A command that reads the generated docs must refuse a cache of an older format with an
  error naming the version found, the version it reads and `retrodoc generate --force`.
- **CFV-3** (deterministic) A command that reads the generated docs must refuse a cache of a later format with an
  error naming both versions and telling to update RetroDoc.
- **CFV-4** (deterministic) `generate` without `--force` must refuse a cache of an older format, naming
  `--force`, and write no artifact.
- **CFV-5** (deterministic) `generate` must refuse a cache of a later format, `--force` included, and write no
  artifact.

### Contract

- **CFV-6** (deterministic) The format version must be the integer `version` of `.retrodoc/cache/format.json`,
  written before the first artifact of the run, so a run stopped midway leaves it too.
- **CFV-7** (deterministic) A cache with artifacts and no `format.json` must be read as format 1, the format of
  every cache written before the version was recorded.
- **CFV-8** (deterministic) A `format.json` that cannot be read must be refused as CFV-3 refuses a later format,
  naming the file.
- **CFV-9** (deterministic) A rerun on an unchanged repository whose cache had no `format.json` must make no LLM
  call: the format version is not part of the fingerprints (RER-1).
- **CFV-10** (deterministic) The artifacts the readers load (`features.yaml`, `use-cases.yaml`, `domains.yaml`,
  `glossary.yaml`, `rules.yaml`, `scope.yaml`, `behaviour-coverage.yaml`, `reconciliation.yaml`) written in the
  current format must read back and save again to the same text, so a renamed, removed or added field fails a
  test until the format version or its frozen sample is changed on purpose.
- **CFV-11** (deterministic) A command of a single pass (`roles`, `glossary`, `entry-points`, `actors`,
  `business-read`, `business-files`, `contexts`, `brief`, `surface`) must refuse a cache of another format, older or
  later, `--force` included, with the error of CFV-2 or CFV-3, and write no artifact.
- **CFV-12** (deterministic) A command of a single pass that writes an artifact must record the format version as
  `generate` does (CFV-6), before its first artifact.
- **CFV-13** (deterministic) `generate --force` on a cache of an older format must keep no file of that format in
  `.retrodoc/cache/`: before its first artifact it moves every file of the cache to
  `.retrodoc/cache-format-<found>/`, replacing a directory of that name, and prints that directory and the
  hand-editable files it holds; the run history stays where it is.
- **CFV-14** (deterministic) `scan` and `benchmark` (`--judge` and `--saved-judge` included) must refuse a cache of
  another format, older or later, with the error of CFV-2 or CFV-3, before any LLM call and before reading an
  artifact.

## Scenarios

- **CFV-H1** (happy; CFV-1, CFV-6) Given a repository, when `generate` runs, then `.retrodoc/cache/format.json`
  holds the current version and `render` reads the docs.
- **CFV-H2** (happy; CFV-2) Given a cache written in an older format, when the user runs `retrodoc ui`, then it
  stops with an error naming both versions and `retrodoc generate --force`; after that run, `ui` opens.
- (CFV-7) Given a cache written before this change (no `format.json`), when `render` runs, then it reads the docs
  as format 1, with no error.
- (CFV-7) Given a repository with no artifact, when a later RetroDoc runs `generate`, then it runs and records its
  version.
- (CFV-3) Given a `format.json` of version 99, when `report` runs, then it fails naming 99 and the version it
  reads.
- (CFV-8) Given a `format.json` holding `{`, when `render` runs, then it fails naming the file.
- (CFV-4) Given a cache of an older format, when `generate` runs without `--force`, then it fails before any LLM
  call and the cache is left as it was; with `--force` it runs and records the current version.
- (CFV-9) Given a cache of format 1 with no `format.json`, when `generate` runs on the unchanged repository, then
  it makes no LLM call and writes `format.json`.
- (CFV-11) Given a cache of an older format, when the user runs `retrodoc roles --force`, then it fails naming
  `retrodoc generate --force` and the cache is left as it was.
- (CFV-11) Given a cache of a later format, when the user runs `retrodoc surface`, then it fails telling to update
  RetroDoc.
- (CFV-12) Given a repository with no cache, when the user runs `retrodoc roles`, then `format.json` holds the
  current version.
- (CFV-13) Given a cache of an older format with a hand-edited `roles.yaml`, when `generate --force` runs, then
  `.retrodoc/cache-format-<found>/roles.yaml` holds the edited file, `.retrodoc/cache/` holds only files of the
  current format, and the output names the directory.
- (CFV-13) Given a cache of an older format, when `generate --force --estimate` runs, then nothing is moved.
- (CFV-14) Given a cache of an older format, when the user runs `retrodoc scan`, then it fails naming
  `retrodoc generate --force`.
- (CFV-14) Given a cache of a later format, when the user runs `retrodoc benchmark --judge`, then it fails telling to
  update RetroDoc and calls no LLM.
- (CFV-14) Given a repository with no cache, when the user runs `retrodoc scan`, then it prints the files as before.
- (CFV-10) Given the frozen sample of the current format, when a field of `Feature` is renamed, then the round-trip
  test fails.

## Non-goals

- Upgrading an older cache in place: a format change comes with the refusal of CFV-2 until a migration is worth
  writing.
- A version per artifact: one version covers the cache of a run.
- The files kept outside the cache (`.retrodoc/edits.json`, `.retrodoc/reviews.json`, the run history): each
  carries its own version (`specs/section-edit.md`, `specs/section-review.md`, `specs/run-history.md`).
- Scoring a kept run of an older format: `benchmark` refuses it as the other readers do, the run is generated again
  with `generate --force` first.
- `benchmark-table`: it reads the JSON figures `benchmark --out` wrote, not the cache.
- The frozen inputs of the benchmark replays: they read the artifacts as the pipeline does, with no check.
- Carrying the hand edits of an older cache over to the new format: the user copies them back from the directory
  CFV-13 moved the cache to.
- Removing the directories of older caches: they stay until the user removes them.
