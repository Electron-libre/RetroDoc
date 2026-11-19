---
priority: low
why: 'no reader breaks today, but a run saved by an older version is read silently; cheap ground for the docs bundle'
---
# Write a format version with the saved artifacts and check it at load

# Goal
The readers of the generated docs read the YAML artifacts of `.retrodoc/cache/` with no way to know which version of
RetroDoc wrote them; a field renamed or removed reads as missing (defaults) rather than as an older format.
Expected: the artifacts read by the readers carry a format version, an older one is refused or upgraded with a clear
message, and a change of format is a deliberate, tested step.

# Findings
* 2026-11-07: no format or schema version constant in `crates/`; the edits store already refuses a file "of a later
  format" (SED-13 of `specs/section-edit.md`).
* 2026-11-07: `GeneratedDocs::load` falls back to defaults on an artifact it cannot read (`unwrap_or_default` on the
  use cases, domain map, glossary, rules).

# Approach
1. List the artifacts the readers load (see `one_loader_of_the_generated_docs`) and how each reacts to an unknown
   or missing field today.
2. Candidates, to choose at the plan:
   1. one manifest file per run with the format version, written by `generate`, checked once at load;
   2. a version field in each artifact read outside the pipeline;
   3. on an older format: a refusal naming the command to run again (`render` or `generate`) against an upgrade in
      place.
3. The version must not enter the fingerprints, so a rerun stays a no-op.

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0003, ADR 0005, ADR 0037, ADR 0043.
* `specs/section-edit.md` (SED-13), `specs/rerun.md`.

# Specs
* `specs/cache-format.md` (CFV), new.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-19

## Understanding
The cache carries one format version (`.retrodoc/cache/format.json`, written by `generate`); the one loader of the
generated docs refuses another version with the command to run, `generate` refuses an older cache without `--force`,
and a round-trip test over a frozen sample makes any change of the artifacts' shape a deliberate step. Out of scope:
upgrade in place, the single-pass commands, the benchmark replays. Means: candidates 1 and 3 of the issue (ADR 0050).

## Plan
1. [x] Record and check the version: `Artifact::Format` and `FORMAT_VERSION = 1` in the pipeline, written by
   `generate` before its first artifact; `GeneratedDocs::load` returns a `Result` that refuses an older, later or
   unreadable version, no file = format 1; the CLI readers print the error; verified by CLI tests (`generate` then
   `render`, `ui`/`report` on version 0 and 99, `{`, no file, rerun with no LLM call); makes true CFV-1, CFV-2, CFV-3,
   CFV-6, CFV-7, CFV-8, CFV-9. (autonomous)
2. [x] `generate` refuses an older cache without `--force` and a later one always, before any LLM call and any write;
   verified by CLI tests with the fake LLM; makes true CFV-4, CFV-5. (autonomous)
3. [x] Frozen sample of the format-1 artifacts the readers load (`crates/retrodoc-mcp/tests/fixtures/`), read and
   saved again to the same text; verified by the round-trip test; makes true CFV-10, sets the spec `accepted`. (autonomous)
4. [x] Docs of the whole issue and fixes of its `high` review. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: ddaa5ffbc52237f38a9aeecfde6b15a0f259b6c0
- A cache with no `format.json` is format 1 (question 1 a).
- `generate --force` still refuses a cache of a later format (question 2 a).

## Deliverable 1
- done: `cache_format` (pipeline) records `format.json` in `generate` before any artifact; `GeneratedDocs::load`,
  `load_with_edits` and `Docs::load` return `Result<Option<_>, FormatError>`; `render`, `report`, `ui`, `history`,
  `mcp`, `search`, `ask`, `chat` refuse a cache of another format.
- red: CFV-H1, CFV-1, CFV-6, CFV-3, CFV-7, CFV-8: `format.json` not written (`No such file`), `report`/`render` did not
  fail on version 99 or `{`; CFV-2, CFV-H2, CFV-9 not run red (`ui` would have served forever on a cache it accepts).
- Code review: 2 findings
  - `record` relabels an older cache as current on a plain `generate`: deliverable 2 (CFV-4), next.
  - `record` overwrites a later or unreadable `format.json`: deliverable 2 (CFV-5), next.
- not verified: `ask_page::each_browser_session_keeps_its_own_conversation` failed once in `project-check`, passed
  3/3 with and without the change (flaky, unrelated).

## Deliverable 2
- done: `open_workspace` of `generate` checks the format before `--force` clears anything: an older cache needs
  `--force`, a later or unreadable one is refused always; fixes both findings of deliverable 1.
- red: CFV-4, CFV-5: `generate did not fail` on a cache of format 0, 99 and `{`.
- Code review: 2 findings
  - two doc summaries over `open_workspace`: merged.
  - `generate --estimate` is refused on an older cache: kept on purpose, the estimate of a run that would be
    refused counts reused units it would not reuse; said in the doc comment.
- not verified: none.

## Deliverable 3
- done: frozen sample `crates/retrodoc-pipeline/tests/fixtures/cache-format-1/` (trimmed from a `delivery_router`
  run, `scope.yaml` and `reconciliation.yaml` written by hand) read by the real loaders and saved again to the same
  text; `RETRODOC_BLESS=1` refreshes it; spec `accepted`, ADR 0050 `Accepted`.
- red: CFV-10: `the shape of use-cases.yaml changed` (the sample as PyYAML wrote it); once blessed, every file parses
  to the same values as before, so the loaders drop no field of the sample.
- Code review: no findings
- not verified: none.

## Deliverable 4 (docs of the whole issue, `high` review)
- done: README names `format.json`; a cache with no `format.json` and no artifact is read by any version (no refusal
  of a first `generate` after a bump); `render` checks the format, then says "no docs" before reading the edits again;
  the bless of the sample starts from the latest earlier sample after a bump; the CLI tests read `FORMAT_VERSION`;
  `record` reads the file once.
- red: CFV-7: `a_cache_with_no_artifact_is_read_by_any_version` written with the fix (the bump it guards cannot be
  run through the binary).
- `high` review: 9 findings
  - a missing `format.json` counts as format 1 with no cache at all: fixed (`holds_artifacts`), CFV-7 scenario added.
  - single-pass commands write the cache unchecked: `issues/cache_format_in_the_single_pass_commands.md`.
  - bless cannot write the sample of a bumped version: fixed.
  - `--force` on an older cache keeps the hand-editable files under the new stamp: same follow-up issue.
  - `--force` clears the caches before the edits check: not fixed, the order predates this issue.
  - `render` reads the edits before saying "no docs": fixed.
  - `use-cases.yaml` parsed twice by `render` and `report`: not fixed, it predates this issue.
  - CLI tests hard-code the version: fixed.
  - `record` reads the file twice: fixed.
- Code review: no findings
- not verified: the bless path of a bump (no bump to run it on).
