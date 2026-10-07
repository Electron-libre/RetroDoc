# 0020. Cover every observable behaviour, not every file; build domains from entry points and entities

Status: Accepted (supersedes 0004; refines 0008 and 0011, which stay in force)

## Context

ADR 0004 requires every source file to land in exactly one domain. That invariant drives most of the
cost and part of the quality problem:

* The repo map summarizes every file (one call per file or per batch of small files) and every directory,
  because the clustering works on directory summaries. On the Rails test repository, 325 source files took
  about 2h20 locally for the whole `generate`; the ~2,300-file repository is out of reach (`PLAN.md` §7.2).
* The summaries are one or two sentences on the "probable role" of a file, technical by construction, and
  they are the only input of the features pass.
* Technical files (helpers, infrastructure, base classes) must go into a business domain or into the
  "uncategorized" bucket, which then produces features and use cases nobody asked for.
* A directory that mixes several business areas cannot be split (ADR 0004 consequences).
* The same file is read by the LLM up to five times: glossary, entry points, repo map, use cases,
  confidence.

Functional documentation describes what users and systems can do. That is the set of entry points (HTTP
routes, CLI commands, jobs, consumers, webhooks, a library's public API) and the entities they act on, not
the set of files.

## Decision

* **New invariant**: every entry point found by the surface belongs to exactly one use case of one
  feature; files are documented only through the behaviours that reach them. Coverage is still enforced by
  construction (unassigned entry points are placed by a small extra call, then go to an uncategorized
  bucket), and reported.
* **Domains** are clustered from the entities and the entry points grouped by resource, with the product
  brief (ADR 0019) as frame, not from directory summaries. **Features** are groups of entry points of a
  domain; **use cases** start from an entry point and its slice (`slices.rs`, already built in phase 7).
* Files reached by no entry point and holding no entity are "technical support": listed in the report,
  absent from the functional docs, and never summarized.
* **The LLM repo map becomes optional.** Its role (a structural overview and ranking) is taken by a
  deterministic map: symbols and references (ADR 0022) ranked by the reference graph, as `ranking.rs`
  starts to do. A file that must be read by the LLM is read once for all the facts the surface needs
  (entities, entry points, outputs, rules).
* The confidence check becomes mostly deterministic (the identifiers a step names appear at the lines it
  cites, the entities it names are in its slice); the LLM verdict is kept for a sample and for the doubtful
  cases.

Rejected: keeping the file invariant and only making the repo map cheaper (batching, concurrency, budget
are already delivered and do not change the order of growth), and RAG over the whole code base (adds
state and non-determinism to a pipeline whose re-runs must be no-ops, `PLAN.md` §7.3).

## Consequences

* The cost grows with the business surface (entry points, entities, use cases), not with the repository.
  On a Rails monolith most files (views, config, infra, helpers) get no call.
* The quality now depends on the entry point pass finding the behaviours: a missed job or consumer is a
  missed use case. The debt report must list the files reached by nothing, so a gap is visible.
* Libraries need their public API as entry points (already allowed by the surface model, ADR 0008).
* `domains/coverage.rs`, `repo_map/` and the features fingerprints change shape; the incremental re-run
  (ADR 0005) must be rekeyed on entry points and entities.
* Follow-up issue: `issues/behaviour_coverage_domains.md`.
