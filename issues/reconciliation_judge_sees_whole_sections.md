# Let the reconciliation judge read the part of a long section that matters

# Goal

The judge of the reconciliation (`reconcile/judge.rs`) is shown the first 1,500 chars of each matched doc
section (`MAX_SECTION_CHARS`). Long sections are the ones BM25 matches most, so most answers are given on a
cut text: a section that documents the item after its first 1,500 chars is answered `unrelated`, and the first
part alone can be answered `covers` on a field name. The judge should see the part of the section the item is
about, with the request still bounded.

# Findings

* 2026-10-26, linkding, `deepseek-chat`, measured in `issues/done/reconciliation_recall_on_real_docs.md`
  (deliverable 3): 16 of the 113 doc sections are over 1,500 chars, and they hold 126 of the 196 matches.
* `api.md#Bookmarks` (4,700 chars) lists the bookmark endpoints with bold labels, not headings: its `DELETE`
  endpoint is past the cut, and "remove a bookmark" was answered `unrelated` on it.
* False `covers` on the same section for tags, bundles and favicons: its first part shows a `tag_names`
  field, a `bundle` filter and a `favicon_url`.
* Since REC-2 falls back on the name, 285 items are matched instead of 119, so more answers rest on these
  long sections.

# Approach

1. Measure first: on the saved linkding run, list the answers given on a section longer than the cut and
   label by hand whether the text the item needs was past it.
2. Candidates (can be combined):
   1. Send the window of the section that holds the most query words of the batch's items, instead of its start.
   2. Split a long section on its bold labels or paragraphs into sub-sections before the match, so each part
      is matched and judged on its own (changes REC-1 and the `path#heading` origin).
   3. Raise the cut for a batch with few sections, keeping the total of a request under a fixed budget.
3. Compare the answers on the labelled sample before and after, with the hosted and the local model.

# Specs

* `specs/docs-reconciliation.md`: REC-4 (what the judge is shown), REC-1 if sections are split.

# Resources

* `crates/retrodoc-pipeline/src/reconcile/judge.rs` (`prompt`, `MAX_SECTION_CHARS`), `reconcile/mod.rs`
* `issues/done/reconciliation_recall_on_real_docs.md` (Findings, labels and replay tool kept with the saved run)
* ADR 0033

# Hints

* A section is listed once per batch and shared by several items: a window chosen per item breaks that.
* The verdict fingerprint holds the section's text: a change of what is sent should change it, or the saved
  verdicts are reused on a text the judge never saw.
