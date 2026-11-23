# 0053. The computations the readers make at read time live in core

Status: Accepted

## Context

Each time it loads the generated docs, the read model (`retrodoc-mcp`, and through it `retrodoc-web`, `ask` and
`report`) computes the reconciliation with the collected docs, the debt report, the domain models and the search ranks
with BM25, all functions of `retrodoc-pipeline`. They cannot be saved whole at `generate` time: the docs as shown hold
the edits made after the run (SED-39, SED-40), and an edit changes the items matched, the report and the ranks. Only
the verdicts of the LLM are saved (`reconciliation.yaml`), keyed by the texts they were given on. No computation needs
the LLM or the sources: BM25, the matching of the items with the doc sections and the report read core types only,
except the split of the collected Markdown docs into sections, which uses `retrodoc-ingest`.

## Decision

`retrodoc-core` holds BM25, the deterministic part of the reconciliation (its types, the items, the matching, the
verdicts applied from the saved `Judgements`, with their load and save) and the debt report with the domain models. The split
of the collected docs into sections moves to `retrodoc-ingest` (`doc_corpus`), next to the Markdown parsing it uses,
and returns the core type. A verdict's fingerprint holds the hash of the judge's prompt and schema, which only the
pipeline has: `reconciliation.yaml` records the two values the pass fingerprints with (`shown_rule`, `request`, REC-17),
and core fingerprints from them. The LLM judge of the reconciliation stays in the pipeline (its module `reconcile`), which saves the `Judgements`; `generate` and
the readers call the same core functions, so a rerun of a saved run finds the same fingerprints and calls no LLM.

Rejected: moving the judge's prompt and schema to core to fingerprint there (a prompt of a pass in the shared model);
saving the report or the reconciliation as artifacts at `generate` (an edit made after the run would not
show); moving them to the docs crate of `docs_crate_out_of_the_mcp_crate` (it does not exist yet, and `generate`
needs the matching too, which a reader crate should not offer the pipeline); moving the whole reconciliation to
`retrodoc-ingest` (ingest reads the repository, it does not compute on the generated docs).

## Consequences

The readers no longer import the reconciliation, the report, the domain models or BM25 from the pipeline; their other
uses of it (the run history, the usage log) are the issue `run_history_out_of_the_pipeline`. The pipeline reaches the
moved code through core, so the passes keep their behaviour. `reconciliation.yaml` gains two optional fields, read by
an older reader as before: an older file gets the values of the running version from the next `generate`, the pass
on or off, with no LLM call, and until then the readers warn and its verdicts show as `matched`. Core names itself `retrodoc_core` (`extern crate self`) so the modules moved from the
pipeline kept their imports and moved unchanged; each move was prepared in place first, `deliverable-size` counting
only an exact rename once.
