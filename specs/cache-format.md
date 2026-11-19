---
id: CFV
status: accepted
adrs: [0003, 0005, 0037, 0043, 0050]
---

# Format version of the saved artifacts

## Intent

The commands that read the generated docs back (`render`, `report`, `history`, `ui`, `mcp`, `ask`, `chat`, `search`)
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
- (CFV-10) Given the frozen sample of the current format, when a field of `Feature` is renamed, then the round-trip
  test fails.

## Non-goals

- Upgrading an older cache in place: a format change comes with the refusal of CFV-2 until a migration is worth
  writing.
- A version per artifact: one version covers the cache of a run.
- The files kept outside the cache (`.retrodoc/edits.json`, `.retrodoc/reviews.json`, the run history): each
  carries its own version (`specs/section-edit.md`, `specs/section-review.md`, `specs/run-history.md`).
- The commands of a single pass (`roles`, `glossary`, `entry-points`…) and the frozen inputs of the benchmark
  replays: they read the artifacts as the pipeline does, with no check.
