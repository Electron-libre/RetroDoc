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
