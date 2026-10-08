# 0019. Understand the product first, from the non-code evidence, then document it

Status: Accepted

## Context

The pipeline climbs from the code to the business: file summaries, then directories, then domains, then
features, then use cases. No pass asks what the application is for, who uses it or what it is worth to
them, so every pass works locally (one domain, one feature) with no global frame, and names and framing
stay technical (`PLAN.md` §7.1).

The sources that carry the most business meaning per character are barely used:

* Existing docs: by default only `docs/` and `README.md`, and only the first non-empty line of each file
  reaches a prompt (the domain clustering, `domains/prompt.rs`). Their full content only feeds the cache
  fingerprint. Features, use cases, glossary and actors never see a document.
* Git history: only counts and dates (`FileHistory`: commit count, authors, first and last date). Commit
  messages are not collected.
* Tests: their descriptions are extracted mechanically (5,352 phrases on the Rails test repository) and
  stored in `glossary.yaml`, but no LLM pass reads them.
* Database schema and migrations, i18n files, view templates, Cucumber `.feature` files, OpenAPI specs and
  changelogs are not read as sources of their own.

## Decision

Add a step before the surface passes, in two parts:

1. **Signal collection**, deterministic, no LLM: the sections of the existing docs, commit subjects
   (deduplicated, `feat`/`fix` first when the repository uses Conventional Commits), manifest metadata
   (name, description, dependencies), schema and migration table names, i18n keys and values, test
   descriptions, `.feature` scenarios and the tree two levels deep. Each signal keeps its origin (file,
   commit) so it can be cited.
2. **Product brief**, one to three LLM calls over a bounded sample of those signals: what the application
   does, for whom, its main business objects, candidate capabilities, external systems and open
   questions, each claim citing its signals. Saved as `.retrodoc/cache/product.yaml`, reused while its
   input is unchanged and **meant to be corrected by hand**, like `roles.yaml`.

The brief becomes the stable first part of the prompts of the later passes (roles, glossary, entry
points, actors, domains, features, use cases). The full signals are not pasted into every prompt: each
pass retrieves the doc sections, commits and test phrases relevant to its unit with the BM25 index that
`retrodoc-mcp` already has, moved or shared where the pipeline can reach it.

Rejected: sending the whole docs to every call (does not fit a 32k-context local model, and costs on
every unit), and keeping the docs as titles only (the current state, which loses most of their meaning).

## Consequences

* A human can steer the whole run with a few lines in `product.yaml`, which is cheaper than any prompt
  tuning.
* A few calls more per run, with a stronger model if the per-pass model setting exists (ADR 0023).
* `retrodoc-ingest` gains a commit-message reader and the new signal readers; the history pass stays one
  revwalk.
* The BM25 code moves out of `retrodoc-mcp` (or into a shared crate) without breaking the one-way crate
  graph (ADR 0001).
* The brief fingerprint becomes an input of the downstream fingerprints: editing the brief invalidates the
  passes that used it, as it should.
* Measured on 2026-10-08 (`issues/product_brief.md`): the brief alone is neutral on coverage and slightly
  better in business language with a local model; the retrieval per unit made the features pass produce about
  20% fewer features with no gain. It is therefore off by default (`brief.evidence`) and to be tried again
  once the product is stable (`issues/brief_injection_experiments.md`).
* Follow-up issues: `issues/quality_benchmark.md` (to measure the gain), `issues/product_brief.md`,
  `issues/brief_injection_experiments.md`, `issues/docs_reconciliation_debt_report.md`, `issues/locate_business_files.md` (takes the brief as
  input).
