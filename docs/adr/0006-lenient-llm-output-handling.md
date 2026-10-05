# 0006. Lenient LLM output handling: one retry, skip with a warning, validated citations

Status: Accepted

_Retroactive ADR, reconstructed from the history (c664ac1, 4e79e0d, f9306f5, 2c607a8, e75c318; 2026-09-30 to 2026-10-03)._

## Context

Smoke tests with a local model showed that answers are often almost right: JSON wrapped in code fences
or prose, empty answers, file paths shortened (`src/a.rs` for `crate/src/a.rs`) or copied with a
`(part 1/2)` marker, prompts with several files rejected. Strict handling scored use cases at 0% or
silently produced features without use cases.

## Decision

`response.rs` is the single entry point: `complete_text`, and `complete_json` which takes the first JSON
value (fences and prose tolerated) and retries once. A unit whose answer is still unparseable or empty
is skipped with a warning (the raw answer goes to debug level), never fatal. Everything the LLM cites is
validated against the real data: paths are resolved by exact match, then unique suffix
(`resolve_cited_path`), after stripping part markers; sloppy steps and references are dropped; names
across passes are matched with the one `naming::normalize`. A batch the model rejects is retried file
by file. Failing the run on the first malformed answer was rejected.

## Consequences

A run finishes despite flaky models, at the price of silent-ish gaps: warnings must be read, and the
debt report surfaces features without use cases. Validation code is shared, so a stricter or looser rule
changes every pass.
