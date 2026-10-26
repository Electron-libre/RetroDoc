# 0033. Reconcile with the existing docs: a lexical match, then an LLM judge

Status: Proposed

## Context

The first feature of `PRODUCT.md` is to identify the missing documentation. The debt report is built from the
saved artifacts only: it says which sections are weak, never what the project's own documents already cover,
miss or contradict. The only comparison with the existing docs is a lexical check of the business rules
(a rule sharing half of its words with no doc section is called undocumented), which can't tell a section that
describes a rule from one that states the opposite, or from one that merely shares its words.

## Decision

Reconcile every domain, feature, use case and business rule with the sections of the existing docs in two steps:

1. **Match**, deterministic: the BM25 index of the pipeline over the doc sections (`doc_sections`, the generated
   docs left out, `docs/ARCHITECTURE.md` §6) returns at most three sections per item. No section: the item is
   undocumented, with no LLM call.
2. **Judge**, a new `reconcile` pass at the end of `generate`: the matched items go to the LLM in batches, each
   section answered `covers`, `contradicts` or `unrelated`. The verdicts are cached by the fingerprints of the item,
   of its sections and of the model, and saved in `.retrodoc/cache/reconciliation.yaml`.

`report` and `render` read that file and stay LLM-free; without it, a matched item is shown as matched, not judged.
The lexical rule check is folded into this one.

Rejected: an LLM call per item over the whole docs (does not fit a small context, costs on every item even when
nothing matches), and the lexical match alone (it can't see a contradiction, and calls any shared vocabulary
documentation).

## Consequences

* One more pass, cheap when the docs are thin: no call when nothing matches, none on a rerun with unchanged inputs.
* The verdict depends on BM25 recall: a doc that paraphrases with other words is missed (undocumented by mistake).
* A contradiction is a finding to check, not a correction: the doc may be out of date rather than wrong.
* The render gains the location of the docs dir, to link the matched sections from the item pages.
