# 0045. The human edits are an overlay outside the cache

Status: Accepted

## Context

The milestone "Edit the docs" lets a team correct the name, the description and the narrative of a domain, a
feature or a use case. The generated sections live in the artifacts of `.retrodoc/cache/`, which each `generate`
rewrites and `generate --force` clears. A correction is a human's work: no rerun can redo it. The reviews already
face the same constraint and live in `.retrodoc/reviews.json` (ADR 0044). An edit made on a section that a later run
rewrites, or whose code changes, must be told outdated, which needs the generated text it corrected.

## Decision

The edits are one JSON file, `.retrodoc/edits.json`, outside the cache, keyed by the kind and the id of the section
(RHI-7), in a stable order, owned by `retrodoc-mcp` (module `edits`, next to `reviews`). An edit holds only the fields
it changes, its author (the git `user.name`), its date, the fingerprint of the generated section and the hashes of the
files it cites. It is an overlay: the artifacts keep the generated text, and the readers (read model, `render`, MCP)
apply the edits over it. The file is loaded and saved by the same code as the reviews (written whole aside then
renamed, an unreadable or later file reported and never overwritten, one save at a time in a process).

Rejected: writing the edit into the artifacts (the next `generate` rewrites it, and the generated text an outdated edit
is compared with is lost); the edits in `reviews.json` (a review and an edit are two records with their own life: a
review of an edited section judges the edited text); one file per edit (as ADR 0044 rejects it for the reviews).

## Consequences

Each reader of the docs must apply the edits, or it shows the generated text: `issues/done/edits_in_read_model.md` does
it for the read model, the search, `render` and the MCP answers. A team versions two small files of human work under
`.retrodoc/`. The fingerprint of an edit is the one of the generated section, so it stays the same until a run
rewrites that section.
