---
priority: low
depends_on: [reconcile_across_languages]
why: 'only long sections matched through identifiers lose their paragraph; the measure says how often before any fix'
---
# Show the judge the paragraphs an identifier matched

# Goal

A section matched to an item only through its identifiers (`specs/docs-reconciliation.md` REC-2, docs in another
language than the generated ones) is shown to the judge by the paragraphs BM25 ranks against the item's query
(REC-15). A translated query shares no word with the section, so a long section is shown from its start, cut to its
share, and the paragraph the identifier pointed to can be left out: the judge then answers `unrelated` and the item
is reported undocumented.

# Findings

* 2026-11-02, found by the `high` review of `reconcile_across_languages`; not measured yet.
* `crates/retrodoc-pipeline/src/reconcile/judge.rs` ranks the paragraphs with the item's query alone.

# Approach

Measure first: on the linkding labels, with the items' text blanked (the replay tool's `blank` mode), how many
judged sections are longer than their share and lose the labelled paragraph.

1. Rank the paragraphs with the item's identifiers when its query holds no word of the section.
2. Rank them with the query and the identifiers together.

# Specs

* `specs/docs-reconciliation.md`: amends REC-15.

# Milestone

Docs in the reader's language

# Resources

* `crates/retrodoc-pipeline/src/reconcile/judge.rs`, `crates/retrodoc-pipeline/src/reconcile/mod.rs`.
* `specs/docs-reconciliation.md` (REC-2, REC-15), ADR 0033.
* The measures ledger: the linkding shown run and the reconciliation replay tool (modes `blank`, `prompts`).
