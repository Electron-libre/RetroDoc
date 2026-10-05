# 0009. Business narrative, derived actors and a deterministic business-language score

Status: Accepted

_Retroactive ADR, reconstructed from the history (2cf41f4, 612fac6, 011d484; 2026-10-02)._

## Context

Even with the surface (0008), use cases listed `human`/`system` actors and function-level steps, and the
confidence score (0007) favours literal paraphrase. The output must read as functional documentation.

## Decision

- **Actors** are business roles derived by a dedicated pass (`retrodoc actors`) from authorization code
  and user-like entities, saved in `actors.yaml`, reused unless the input changes or `--force`;
  use cases take their `primary_actor` from it.
- **Two output levels**: each use case gets a short business `narrative` (rendered first), prompted with
  the top 40 application entities as vocabulary; the technical steps are folded into a `<details>` block
  when a narrative exists. Old artifacts without narrative still render.
- **Business-language score**: a deterministic pass (`vocabulary.rs`, no LLM, recomputed each run)
  penalizes code-level wording (identifiers, paths, technical terms) and missing references to known
  actors or entities, stored next to confidence and reported as a debt criterion.

Asking the LLM to judge "is this business language?" was rejected: it would cost calls, vary between
runs and be graded by the generator's own kind of model.

## Consequences

Business quality is measurable and free to recompute. The heuristic depends on the glossary and actors
being good, and can be gamed by vocabulary stuffing. The score is a hint, not a truth.
