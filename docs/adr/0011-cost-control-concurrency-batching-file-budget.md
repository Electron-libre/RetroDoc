# 0011. Cost control: estimate, concurrency, batching and an opt-in file budget

Status: Accepted

_Retroactive ADR, reconstructed from the history (3843170, c9f4d49, 7e4040f, 5661691, 09e00fe; 2026-10-02 to 2026-10-03). Phase 8 of `PLAN.md`._

## Context

On a repo of thousands of files the repo map alone is out of reach for a local LLM (one call per file
and per directory). Phase 8 sought to make a full run feasible without silently reducing quality.

## Decision

Six independent, explicit levers, none changing the default behaviour except the estimate:

- `generate` prints `estimate_repo_map`'s expected calls and characters before starting; later passes
  depend on its output and are not estimated.
- `llm.concurrency` (default 1) parallelizes file and directory summaries (buffered streams), only for
  the repo map.
- `llm.batch_chars` (default 6000, 0 = off) batches small files in one call; a file over a quarter of the
  limit goes alone.
- `--max-files N` / `ingest.max_files` keeps the best-ranked subset (`ranking.rs`: role × git churn ×
  references); the rest becomes `FileKind::Other`, saved in `scope.yaml` and listed in the debt report as
  "Not analysed (file budget)".
- `--no-confidence` and `--confidence-sample N` (0007).
- Incremental caches and resumption (0005).

An automatic budget was rejected: dropping files without the user asking would hide documentation debt.

## Consequences

The user trades coverage for cost knowingly, and the report states what was left out. Concurrency raises
the risk of 429s, handled by 0012. Token and cost accounting (PLAN §7, item 7) is documented but not
implemented.
