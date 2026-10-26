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

1. Doc sections as units (from `issues/done/product_brief.md`), indexed with the shared BM25.
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

# Tracking

Plan: validated

## Understanding
Mark every domain, feature, use case and rule as documented, undocumented or contradicted against the existing doc
sections, in the debt report and on the item pages. Out of scope: correcting any doc, semantic retrieval, MCP tools.
Means: the pipeline's own BM25 (already shared with `retrodoc-mcp`); `report` stays LLM-free and reads the verdicts
of a new cached `reconcile` pass (spec `specs/docs-reconciliation.md`, ADR 0033).

## Plan
1. [x] Match and report: retrieval of the sections per item, deterministic verdicts (`undocumented`, `matched`,
   rule doc evidence), coverage per domain, ranked undocumented list replacing the rule section; verified by
   pipeline unit tests and a `report` CLI test; makes true REC-1, REC-2, REC-3, REC-9, REC-10, REC-13.
2. [ ] `reconcile` pass: batched LLM judgement, lenient parsing, fingerprint cache, `reconciliation.yaml`,
   `[pipeline] reconcile` and pass name, contradictions in the report; verified by tests with a fake provider;
   makes true REC-4, REC-5, REC-6, REC-7, REC-8, REC-11.
3. [ ] Item pages link their doc sections: relative links from the rendered pages, rule lines; verified by render
   tests; makes true REC-12.
4. [ ] Smoke test on the default smoke repo (`skill:smoke-test`): verdicts sane, rerun calls no LLM; measure only.

## Decisions
- Mode: autonomous
- Base commit: fdc584f8c081419e3957ae646335139c5d54bcad
- `[pipeline] reconcile` on by default (no call when nothing matches).
- Query: name + description, half of the content words (the rule check of today).
- The rule section of the report is replaced by the common lists.

## Deliverable 1 summary
- done: `reconcile/` (items, corpus, BM25 match, deterministic statuses), report section "Existing documentation"
  (per domain, ranked undocumented list), replaces `with_rules`; CLI `add_doc_debt` for `report` and `render`.
- checks: `just check` ok, `just test-harness` ok, docs `docs/crates/pipeline.md`.
- red: `no method named with_reconciliation found for struct DebtReport`.
- Code review (low): duplicated `origin` formatting → `Section` holds a `SectionRef`, fixed; empty
  reconciliation printed a bare table → section left out, fixed and tested.
- not verified: on a real repo (deliverable 4).
