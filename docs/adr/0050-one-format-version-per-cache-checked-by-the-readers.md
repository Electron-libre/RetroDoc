# 0050. One format version per cache, checked by the readers

Status: Proposed

## Context

The commands that read the generated docs back load the YAML artifacts of `.retrodoc/cache/` (ADR 0003) leniently: a
field renamed or removed reads as its default, a file that no longer parses as nothing. A cache written by another
version of RetroDoc is then shown as a doc with empty parts, with no message. The files kept outside the cache
(edits, reviews, run history: ADR 0043 to 0045) already carry a `version` and refuse a later one.

## Decision

The cache carries one format version, the integer `version` of `.retrodoc/cache/format.json`, written by `generate`
before its first artifact. The one loader of the generated docs checks it before reading anything: another version is
an error naming both versions and what to do (`generate --force` for an older one, update RetroDoc for a later one).
`generate` refuses an older cache without `--force`, since a rerun reuses the saved units (ADR 0005) and would keep
them in the old format. A cache with no `format.json` is format 1. The version is not part of the fingerprints
(ADR 0037), so recording it costs no LLM call. A round-trip test over a frozen sample of the artifacts the readers load
fails on any change of their shape, so a change of format is a decision: bump the version, or refresh the sample when
the change is compatible.

Rejected: a version field in each artifact (eight files to change and check, and a run writes them at different
times); an upgrade in place (no migration exists yet; it can come with the first bump that needs one).

## Consequences

A format bump forces a paid `generate --force` on every repository; a compatible change (a new field with a default)
only refreshes the sample. The commands of a single pass and the benchmark replays still read the cache unchecked.
