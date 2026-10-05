# 0010. Deterministic Mermaid diagrams and idempotent, diff-first rendering

Status: Accepted

_Retroactive ADR, reconstructed from the history (c664ac1, adc956b; 2026-09-30 to 2026-10-01)._

## Context

Documentation is written into the analyzed repository's `docs/`, so it ends up in code review. Rerunning
the tool must not create noise, must never destroy human work, and diagrams must not be one more place
where the LLM can hallucinate.

## Decision

- The Mermaid `sequenceDiagram` of each use case is built mechanically from its validated steps and
  actors: no LLM call (`diagrams.rs`).
- `retrodoc-render` works in three stages: `render()` builds all files in memory, deterministically;
  `plan()` compares them with the disk (created/updated, unified diff); `WritePlan::apply()` writes only
  what differs. `--dry-run` stops after `plan()`, and `retrodoc render` regenerates the files from the
  cached artifacts without any LLM call.
- `_retrodoc/run-metadata.json` carries a timestamp that is ignored when it is the only change, so a
  rerun is a no-op.
- Stale files under `functional/` are reported but never deleted. Docs already generated
  (`functional/`, `_retrodoc/`) are dropped from the existing docs the CLI ingests, so the tool does
  not feed on its own output.

## Consequences

Reruns are clean in Git and diagrams are consistent with the text. The diagrams are only as rich as the
steps. Users clean stale files themselves.
