# 0044. The human reviews live in one file outside the cache

Status: Accepted

## Context

The milestone "Human review of the docs" lets a reader mark a domain, a feature or a use case validated, wrong or to
review. ADR 0039 made the web interface read-only "until the review and edit milestones": a review is the first
thing RetroDoc writes outside a run. Everything a run writes lives in `.retrodoc/cache/`, which `generate --force`
clears; the judgement of the benchmark already lives outside it (`.retrodoc/benchmark/judge.yaml`) so that it
survives. A review is a human's work: losing it to a `--force` would lose what no rerun can redo. A section is
already identified across runs by its kind and its slug (RHI-7 of `specs/run-history.md`).

## Decision

The reviews are one JSON file, `.retrodoc/reviews.json`, outside the cache, keyed by the kind and the id of the
section, in a stable order so that a team can version it and read its diffs. `retrodoc-mcp` owns it (module
`reviews`, next to `history`): the web form, the debt, the dashboard and the MCP server already read the docs
through it. A save writes the whole file aside then renames it over the former one; a file that cannot be read or
is of a later format is reported and never overwritten. The author is the `user.name` of the repository's git
config: the interface runs locally, for the person at the keyboard. Each review keeps a fingerprint of the section
as it was reviewed, so a later run can tell the review outdated.

Rejected: one file per review (a team's diff spreads over many files, and listing the reviews of a page reads them
all); a field in the artifacts of the run (rewritten by each `generate`); an author typed in the form (an extra
field at each review, and nothing to check it against anyway).

## Consequences

`retrodoc-mcp` gains a second writer besides the run history, and `retrodoc ui` will write through it. Two writers
at the same instant keep one of the two files whole, not both: acceptable for a local, single-user interface. The
fingerprint changes whenever any field of the section changes, the confidence included: the issue that flags an
outdated review decides what it compares.
