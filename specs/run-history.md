---
id: RHI
status: accepted
adrs: [0039, 0043]
---

# Run history

## Intent

Each `generate` that changes the docs leaves a small snapshot of its figures and its sections, kept with all the
earlier ones, so the pages of the run history (the trends of the audit page, the difference between two runs) tell
whether the docs get better, after the artifacts of the last run were rewritten.

## Requirements

- **RHI-1** (deterministic) A `generate` that finishes must write its snapshot as one new JSON file under
  `.retrodoc/cache/history/`, named by the end date of the run, unless RHI-9 or RHI-10 says it writes none.
- **RHI-2** (deterministic) The snapshot must hold the commit of `HEAD` when the run started, or none when the
  repository had no commit, and the end date of the run in RFC 3339.
- **RHI-3** (deterministic) The snapshot must hold the overall confidence and the overall business language of the
  run as the audit page shows them (AUD-1), each absent when the run lacks it.
- **RHI-4** (deterministic) The snapshot must hold the coverage of the entry points as the audit page counts it
  (AUD-2), each of its two shares absent when the run lacks it.
- **RHI-5** (deterministic) The snapshot must hold the number of debt items of the debt view (`specs/debt-view.md`)
  per kind and domain, the kinds with no domain (DBT-22) under no domain, and no item of the kind "stale" (DBT-7).
- **RHI-6** (deterministic) The snapshot must hold the cost of the run: its duration, its calls and its prompt,
  cached and completion tokens summed over its passes, as recorded in `.retrodoc/cache/usage.json` (AUD-4); none
  when the run called no LLM.
- **RHI-7** (deterministic) The snapshot must hold one entry per domain, feature and use case of the run, in the order
  of the run, each with its kind, its id (the slug, prefixed by the slug of its feature for a use case), its name and
  its confidence.
- **RHI-8** (deterministic) The entry of a use case must also hold its business language, its narrative check and the
  reasons it is set apart under "Administration and technical" (DBT-6), each absent or empty when it has none.
- **RHI-9** (deterministic) A `generate` must write no snapshot when its snapshot equals the newest one saved but for
  the commit, the end date and the cost (a no-op rerun).
- **RHI-10** (deterministic) A `generate` that fails, or runs with `--dry-run`, must write no snapshot.
- **RHI-11** (deterministic) A `generate` must not delete nor rewrite a saved snapshot, `--force` included.
- **RHI-12** (deterministic) A snapshot that cannot be written must not make the `generate` fail: a warning says so.
- **RHI-13** (deterministic) The saved snapshots must be readable as one list, oldest first, a file that cannot be
  read or is of a later format left out of it.

## Scenarios

- (RHI-1) Given a snapshot already named by the same second, then the new one gets a name of its own and both are kept.
- (RHI-5) Given an item "entry point without a feature", then it is counted under no domain.
- (RHI-2) Given a commit made while the run asks the model, then the snapshot holds the commit before it.
- (RHI-9) Given a newest file that cannot be read, then the snapshot is compared with it, not with an older one, and
  is written.
- (RHI-10) Given a `generate` that fails after an earlier run left its docs, then no snapshot of those docs is written.
- (RHI-9) Given a rerun that calls no LLM and changes no artifact, then the history keeps the same files.
- (RHI-9) Given a rerun after a new commit that changed no section nor figure, then no snapshot is written.
- (RHI-9) Given a run whose only change is the confidence of one use case, then a snapshot is written.
- (RHI-9) Given no snapshot saved yet, then the first `generate` writes one.
- (RHI-13) Given a history file holding invalid JSON, then the list holds the other snapshots.

## Non-goals

- No pruning: every snapshot is kept, a few kB each (chosen with the user on 2026-11-04).
- No stale count: at the end of a `generate` nothing has changed since it.
- No rule: the business rules are counted in the debt (DBT-15, DBT-16) only, not listed as sections.
- No page reads the snapshots yet: the trends (`issues/web_run_trends.md`) and the difference
  (`issues/run_diff.md`) do.
- No snapshot from the other commands (`roles`, `surface`…): they write no section.
