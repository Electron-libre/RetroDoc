# Reconcile the generated docs with the existing ones in the debt report

# Goal

Deliver the first feature of `PRODUCT.md`, "identify missing documentation". Today the debt report lists
low-confidence and technical-sounding sections; it never says what the existing docs already cover, what
they miss, or where they contradict the code. Mark each domain, feature, use case and rule as documented,
undocumented or contradicted, with the doc sections behind the verdict.

# Findings

* `report.rs` builds the report from the saved artifacts only; the existing docs are not an input.
* The existing docs reach the pipeline as titles only (ADR 0019 context).
* `retrodoc-mcp` already indexes the generated docs and the collected Markdown docs with BM25.

# Approach

1. Doc sections as units (from `issues/product_brief.md`), indexed with the shared BM25.
2. Per generated item, retrieve the best matching sections; deterministic verdict when nothing matches
   (undocumented), a small batched LLM call to tell "covers" from "contradicts" when something does.
3. Report: coverage of the existing docs per domain, the undocumented items ranked by importance (entry
   points, confidence), the contradictions with both sources cited; the item pages link the matching docs.
4. Cache the verdicts by the fingerprints of the item and of the matched sections.

# Resources

* ADR `0019`, `PRODUCT.md` (features, "ask the documentation")
* `crates/retrodoc-pipeline/src/report.rs`, `crates/retrodoc-mcp/src/bm25.rs`, `corpus.rs`
* `crates/retrodoc-render/src/markdown.rs`

# Hints

* A doc can be out of date rather than wrong: a contradiction is a finding to show, not a correction to
  apply.
* Keep the generated docs out of the matched corpus (no feedback loop, `docs/ARCHITECTURE.md` §6).
