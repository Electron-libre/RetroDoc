# 0052. Every command that writes the cache keeps its format stamp true

Status: Accepted

## Context

ADR 0050 stamps `.retrodoc/cache/` with one format version, recorded by `generate` and checked by the readers of the
docs. Two paths still leave files of one format under the stamp of another. The commands of a single pass (`roles`,
`glossary`, `contexts`…) write artifacts with no check: an older binary writes files of its format into a cache a
later one stamped. `generate --force` on an older cache keeps the files it does not clear (the hand-editable ones,
`usage.json`), reads them leniently, then stamps the cache with the current version.

## Decision

The commands of a single pass check the format as the readers do, and refuse an older cache even with `--force`:
their `--force` redoes one artifact and would leave the others in the older format. Those that write record the
format before their first artifact. `generate --force` on an older cache moves every artifact of the cache to
`.retrodoc/cache-format-<found>/` before its first write (the run history, versioned on its own, stays) and names the hand-editable files it holds, so the user can
copy their edits back.

Rejected: a list, kept with each bump, of the kept files it changes, `--force` clearing those only (correct only while
every bump remembers to fill it, and no test catches a forgotten one: the round-trip of CFV-10 covers the files the
readers load, not the kept ones); clearing every artifact (the hand edits are lost); an upgrade in place of the kept
files (no migration exists yet, as in ADR 0050).

## Consequences

The first `generate --force` after a format bump also asks the LLM again for the file roles, the brief, the business
files and the bounded contexts (a few calls) and starts a new usage history; the older one stays in the moved
directory. The moved directories are left for the user to remove.
