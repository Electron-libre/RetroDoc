# 0004. Cluster domains by directory, enforce coverage by construction

Status: Accepted

_Retroactive ADR, reconstructed from the history (69227bd, 6d57850; 2026-09-28)._

## Context

`PLAN.md` requires the domain map to cover 100% of the source files with no overlap. The first version
sent every file path to the LLM in one flat list: on a 2,331-file repo that was tens of thousands of
tokens in a single call, and an LLM never returns a perfect partition, so the repair logic became the
normal path.

## Decision

One LLM call clusters the directory-level summaries (`repo_map.modules`, already computed by the repo
map) plus the existing doc titles. Each directory assignment is then expanded to its files
mechanically, by longest-prefix match, with no extra call. `coverage.rs` enforces the invariant by
construction instead of failing the run: files left unassigned go to a synthetic "uncategorized"
domain, a file assigned twice keeps its first assignment, hallucinated paths are dropped, and all
repairs are reported. The root directory is matched as an empty prefix (it used to be printed as `.`,
which never matched anything). Failing the run on an imperfect answer was rejected: the LLM is rarely
perfect and the user would never get documentation.

## Consequences

Prompt size scales with the number of directories, not files. A directory that mixes several business
areas cannot be split across domains. Domain clustering is non-deterministic, which is why its input is
fingerprinted (0005). Phase 7 later replaced the directory-only input with the surface (0008).
