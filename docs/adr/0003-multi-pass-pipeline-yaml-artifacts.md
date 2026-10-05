# 0003. Multi-pass pipeline with YAML/JSON artifacts under `.retrodoc/cache/`

Status: Accepted

_Retroactive ADR, reconstructed from the history (71b8e98, 69227bd, c664ac1, 1fec190, 62fe227; 2026-09-28 to 2026-10-03)._

## Context

A repository does not fit in one prompt, and one failing or non-deterministic LLM call must not
invalidate the whole run. Intermediate results also have to be inspectable and, where useful,
hand-correctable.

## Decision

Generation is a chain of small passes (repo map, domains, features, use cases, diagrams, confidence, then
render), each consuming the saved result of the previous one and persisting its own artifact in
`.retrodoc/cache/` (`repo-map.json`, `domains.yaml`, `features.yaml`, `use-cases.yaml`, later
`roles.yaml`, `glossary.yaml`, `entry-points.yaml`, `actors.yaml`, `scope.yaml`). `artifact.rs` is the
single place for load/save; a missing or unreadable file means a first run, not an error. Each pass is
a module (a directory once it grows) whose `//!` doc is the reference. The alternative, an in-memory
pipeline with one final output, was rejected: no resume, no rerun of a single pass, no way to inspect
or edit what the LLM decided.

## Consequences

Passes can be rerun, resumed and debugged independently, and standalone commands (`roles`, `glossary`,
`actors`…) fall out naturally. Artifact formats are an implicit contract between passes: an old file
missing a new field must still load (use optional fields, see `Feature.source_paths`). The cache
directory is state to be wiped by `generate --force`.
