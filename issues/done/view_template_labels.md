---
rank: 300
why: 'same milestone as `roles_second_pass_and_promoted_sources`, after it: the value of the labels is not measured yet; before the later milestones'
---
# Read the labels of the view templates without a call per file

# Goal
View templates (ERB, Jinja, Blade, JSX and the like) carry the words users see: form fields, headings, buttons. The
walker keeps the template files of unknown extensions as `Other`, and the file budget and the actors pass select on
`Source` only, so no pass reads them; they keep their `view` role for the classification only. Expected: the
user-facing labels of the templates feed the vocabulary of the business passes (glossary, use cases), extracted
mechanically, for any stack.

# Findings (`docs/findings.md` §5)
* On a large Rails repository, 1,253 `.erb` templates stay `FileKind::Other` on purpose; they are given the `view`
  role, used by `classify` only.
* Not measured: whether the labels would change the domains, features or narratives of the benchmark.

# Approach
1. Measure first: on the benchmark repositories, the share of the reference vocabulary found in the templates and
   not in the code the passes already read.
2. Candidates: (a) a generic extractor of the visible text of markup (text nodes, `label`, `placeholder`, `title`
   attributes), no stack table; (b) the labels added to the brief evidence retrieved per unit (ADR 0019); (c) a
   tree-sitter HTML grammar for the templates it parses, the generic extractor elsewhere.

# Milestone
Language and framework agnostic

# Resources
* `crates/retrodoc-ingest/src/walker.rs`, `crates/retrodoc-pipeline/src/roles/`, `crates/retrodoc-pipeline/src/brief/`.
* ADR 0019 (brief and non-code evidence), ADR 0036 (stack knowledge as accelerators only).

# Hints
* Templates are many and large: the extraction must stay out of the LLM calls and inside the prompt budgets.

# Tracking

Plan: validated

## Understanding
Approach step 1 (the measure) is done, without LLM, on the pinned benchmark clones (scripts outside git, in
`~/retrodoc-runs/scratch/view_template_labels/`). The labels add little vocabulary, so the plan records the measure
and closes the issue without code; the extractor (candidates a-c) is the alternative if the user wants it anyway.

Measure (2026-10-30, reference = words of 4+ letters of `benchmark/<repo>/reference.yaml`, crude stemming; visible
text = text nodes and `label`/`placeholder`/`title`/`alt`/`aria-label`/`value` attributes, template tags stripped;
"read" = the business files and the entry-point files of the `source_run` of the frozen inputs):

| repo | templates | reference words | in the files read | in templates, not in the files read | not in any Source file nor doc |
|---|---|---|---|---|---|
| linkding | 46 `.html` | 254 | 74 % | 17 (6.7 %) | 1 (0.4 %) |
| lobsters | 121 `.erb` + 9 `.html` | 274 | 86 % | 21 (7.7 %) | 8 (2.9 %) |
| delivery_router | 0 | 81 | - | 0 | - |

Most of the template-only words are generic (`later`, `whom`, `tell`, `turn`, `older`); the business ones are few
(linkding: `bookmarklet`, `markdown`, `reader`; lobsters: `subscribe`, `aggregation`, `profile`), and nearly all of
them are already in other Source files or the docs that the brief and the per-unit evidence read.

## Plan
1. [x] Record the measure: §5 of `docs/findings.md` (the template item states the figures and why no extractor),
   a line in `~/retrodoc-runs/LEDGER.md`; verified by `just check`; no requirement (measure only).

## Decisions
- Mode: autonomous
- Answer 1 a: record the measure and close, no extractor.
- Base commit: f1475317176bf4e15274ca30ec2be908d9bb864e

## Deliverable 1
- done: the measure is in §5 of `docs/findings.md` (template labels dropped, with the figures) and in the ledger.
- red: none (measure only, no test).
- not verified: the effect on the generated docs (no run; the measure is of vocabulary only).
- decisions needed: none.
- Code review: no findings
- Issue done: closed on the measure; nothing remains.
