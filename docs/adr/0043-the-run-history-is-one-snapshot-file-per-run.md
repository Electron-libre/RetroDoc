# 0043. The run history is one snapshot file per run

Status: Accepted

## Context

The debt report, `/audit` and `/debt` read the artifacts of the last run only, and a `generate` rewrites them in place:
nothing tells whether the docs get better. The trends of the audit page and the difference between two runs need the
figures and the sections of every earlier run. The figures of the audit page and the debt items are computed by
`retrodoc-mcp` from its read model (`audit`, `debt_of`), not by the pipeline; the cost of a run is known by the CLI at
its end (`usage::finish`, `.retrodoc/cache/usage.json`, which keeps the last 20 commands).

## Decision

At the end of a successful `generate` (not `--dry-run`), the CLI loads the read model as `retrodoc ui` does, and
`retrodoc-mcp` (module `history`) builds the snapshot from it with the same functions as the audit page, then writes
it as one JSON file per run under `.retrodoc/cache/history/`, named by the end date: written whole aside, then
linked under a name no file holds, so a run cut short leaves no truncated snapshot and none is ever rewritten. The
commit is the one of `HEAD` when the run started. A snapshot equal to the newest file
but for the commit, the date and the cost is not written, so a no-op rerun stays a no-op. Every snapshot is kept;
`--force` clears none of them.

Rejected: one appended file (each run rewrites a growing file, and a write cut short loses the whole history);
computing the snapshot in the pipeline (the debt kinds and the audit figures live in `retrodoc-mcp`, duplicating them
would let the history and the pages drift apart); extending `usage.json` (it is pruned to 20 runs and holds every
command, the trends need the long run).

## Consequences

A page of the history reads every file of the folder at each request: a few kB per run, a few hundred runs read in
milliseconds. A change of the debt kinds or of the audit figures changes the snapshots written from then on, so the
format carries a version and the readers skip what they cannot read. The `generate` command writes through
`retrodoc-mcp`, which until now only read.
