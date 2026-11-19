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
