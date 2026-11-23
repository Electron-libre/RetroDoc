---
priority: low
depends_on: [reader_types_in_core]
why: 'the audit and history pages read the run history through the pipeline; needed before the docs crate'
---
# Move the run history the readers show into retrodoc-core

# Goal
The audit and history views read `usage.json` and the progress of the contexts through `retrodoc-pipeline`
(`usage_log::{RunUsage, PassRecord, ModelRecord, load_history, record_run, thousands}`, `format_duration`,
`recorded_progress`, `RecordedProgress`). Expected: the types of the history and their loading in
`retrodoc_core::artifacts`, the pipeline recording the runs through them.

# Findings
* 2026-11-19: `RunUsage` embeds `estimate::RunEstimate` (the estimate printed before a `generate`), and
  `RunUsage::new` reads a `retrodoc_llm::UsageReport`: core does not depend on `retrodoc-llm`.
* `recorded_progress` reads `contexts.yaml` (`ContextMap`) and `fingerprints.json` (`Fingerprints`, private to the
  pipeline).
* `usage_log.rs` is 743 lines with its recap text; `format_duration` lives in `progress.rs`.

# Approach
1. Move the records (`RunUsage`, `PassRecord`, `ModelRecord`) with `RunEstimate` or with a type of its saved shape,
   their load and save, and `thousands`, `format_duration`; keep `RunUsage::new` as a pipeline function.
2. For `recorded_progress`: move the shape of `contexts.yaml` and of `fingerprints.json` the reader needs, or save
   the progress itself.
3. Check a rerun of a saved run calls no LLM and rewrites no file.

# Milestone
The readers of the docs out of the pipeline

# Resources
* ADR 0043 (run history), ADR 0051, `specs/run-history.md`, `specs/audit-dashboard.md`, `specs/run-estimate.md`.
* `docs/crates/pipeline.md`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
The readers (`retrodoc-mcp`, `retrodoc-web`) take the run history from the pipeline. Move into `retrodoc_core::artifacts`
the saved shape of the estimate, the usage records with their load and save, and the reading of the contexts'
progress; `thousands` and `format_duration` to `retrodoc_core::text`. What computes stays in the pipeline: the
estimate itself (`estimate`, `RatioTable`, `RunEstimate::render` made a free function), `RunUsage::new` (made a free
function over `UsageReport`), the recap. End state: `retrodoc-mcp` no longer depends on `retrodoc-pipeline` but as a
dev-dependency. No change of behaviour, no spec.

## Plan
1. [x] Estimate shape and number formats in core: `RunEstimate`, `PassEstimate`, `Range`, `Basis` and their pure
   comparison (`Miss`, `Against`, `Measured`, `Figure`) to `retrodoc_core::artifacts::estimate`; `thousands` and
   `format_duration` to `retrodoc_core::text`; `render` a pipeline free function; users import from core; verified by
   the moved tests and `just check`; ~450 lines. (autonomous)
2. [x] Usage history in core: `RunUsage`, `PassRecord`, `ModelRecord`, `KEPT_RUNS`, `load_history`, `record_run` to
   `retrodoc_core::artifacts::usage` with their load/save tests (TOK-5, TOK-6); `RunUsage::new` becomes the pipeline's
   `usage_log::run_usage`; verified by the moved tests and `just check`; ~700 lines. (autonomous)
3. [x] Recorded progress in core: `recorded_progress` and `RecordedProgress` to `retrodoc_core::artifacts::progress`,
   reading private views of `contexts.yaml` (the slugs in order) and `fingerprints.json` (the keys of `contexts`); the
   pipeline test of the progress kept as the guard that the views read what the pipeline writes; `retrodoc-mcp` drops
   `retrodoc-pipeline` from its dependencies; ADR 0051 consequences updated; verified by `just check` and a rerun of
   `generate` on a saved run that calls no LLM and rewrites no file; ~250 lines. (autonomous)
4. [x] Fixes of the `high` review of the whole issue; verified by a new AUD-5 test and `just check`. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: dc3978a8d321aef60453f4e2d480595e29ef6000
- Progress of the contexts: core reads private views of `contexts.yaml` and `fingerprints.json` (answer `1 a`).

## Deliverable 1
done: `RunEstimate`, `PassEstimate`, `Range`, `Basis` and the comparison (`Miss`, `Against`, `Measured`, `Figure`)
live in `retrodoc_core::artifacts::estimate`, `thousands` and `format_duration` in `retrodoc_core::text`;
`RunEstimate::render` became the pipeline's `estimate::render`; the CLI, the bench and `retrodoc-mcp` import them
from core, the pipeline's re-exports of `format_duration` removed.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (no change of behaviour) |
| docs | `docs/crates/core-ingest.md`, `docs/crates/pipeline.md` |
| size | 620 lines |

red: none (a move: the moved tests did not compile in core before it)

Code review: no findings

not verified: nothing

decisions needed: none

## Deliverable 2
done: `RunUsage`, `PassRecord`, `ModelRecord`, `KEPT_RUNS`, `load_history` and `record_run` (now returning
`CacheError`) live in `retrodoc_core::artifacts::usage` with the tests of the saved file (TOK-5, TOK-6);
`RunUsage::new` became the pipeline's `usage_log::run_usage`, its tests checking what the run keeps of the report;
the CLI, the bench, `retrodoc-mcp` and the web tests import the records from core.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (TOK-5, TOK-6 traced by the moved tests) |
| docs | `docs/crates/core-ingest.md`, `docs/crates/pipeline.md` |
| size | ~560 lines |

red: none (a move: the moved tests did not compile in core before it)

Code review: no findings

not verified: nothing

decisions needed: none

## Deliverable 3
done: `recorded_progress` and `RecordedProgress` live in `retrodoc_core::artifacts::progress`, reading private
views of `contexts.yaml` (the slugs in order) and `fingerprints.json` (the keys of `contexts`); the pipeline test of
the progress (AUD-5) calls core's function on the map and fingerprints the pipeline saves; `retrodoc-mcp` depends on
`retrodoc-pipeline` only for its tests; ADR 0051, `CLAUDE.md` and `docs/ARCHITECTURE.md` say so.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (no change of behaviour; AUD-5 still traced) |
| docs | `CLAUDE.md`, `docs/ARCHITECTURE.md`, ADR 0051, `docs/crates/core-ingest.md`, `docs/crates/pipeline.md`, `docs/crates/render-mcp-cli.md` |
| rerun | `generate --no-confidence` on a copy of the saved vingilot run with a bogus key: no LLM call, docs up to date, the 141 files of `.retrodoc/` and `docs/` of identical content; ledger line |

red: none (a move)

Code review: 1 finding
1. The view of `fingerprints.json` typed the hashes as strings, a change of their type would hide every finished
   context: they are strings today (`Fingerprints::contexts`), the view now takes any value and reads only the keys.

not verified: nothing (the looser views of a malformed file: fixed in deliverable 4)

decisions needed: none

## Deliverable 4
done: fixes of the `high` review of `dc3978a..HEAD` (5 findings); `skill:update-docs` over the whole issue found no
stale name left.

| check | result |
|-------|--------|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (AUD-5 gains a test) |
| docs | module docs of `usage_log.rs` and `artifacts/usage.rs` |

red: AUD-5: `a_malformed_map_or_fingerprints_file_reads_the_same_in_core` failed on the loose views (the map without
`name` gave a progress where `ContextMap::load` gives none).

High review: 5 findings
1. The views of `contexts.yaml` and `fingerprints.json` were looser than the pipeline's types, so a hand-edited map
   the pipeline drops still showed in the audit page: fixed, the views type every field as the pipeline does, a
   test checks a malformed map and fingerprints read the same on both sides.
2. `render`'s last `writeln!` misformatted and a blank line lost: fixed.
3. The doc of `artifacts/usage.rs` said the pipeline records the runs: fixed (the CLI saves them).
4. A long line in the module doc of `usage_log.rs`: rewrapped.
5. `impl PassEstimate` before its type in `artifacts/estimate.rs`: reordered.

Code review: no findings

not verified: nothing

decisions needed: none
