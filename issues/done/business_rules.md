# Extract the business rules and document them next to entities and use cases

# Goal

Answer "what is allowed, refused, limited, and in which states" in the docs. The model has no place for
business rules today (`Domain → Feature → UseCase → Step`). Implement ADR 0021: a `BusinessRule` object,
a pass that finds rule-bearing code and states each rule in business language with its evidence, and its
rendering and MCP access.

# Findings

* `crates/retrodoc-core/src/model.rs` has no rule type; rules only appear inside steps or narratives.
* Test descriptions, often rules stated in plain words, are extracted and unused (5,352 phrases on the
  Rails test repository, `PLAN.md` §7.1).
* The actors pass already reads the authorization code (`actors.rs`), a first source of permission rules.

# Approach

1. Measure by hand on `delivery_router` and a part of the Rails test repository: list the rules and where
   they live (validations, schema constraints, policies, state machines, enums, guard clauses, constants,
   tests).
2. Model: `BusinessRule` (statement, kind, entities, use cases, evidence, confidence) in `retrodoc-core`.
3. Evidence per feature, no hard-coded selectors (no list of framework or language patterns): the use case
   slices (numbered, bounded by the entry points), the glossary entities and the `Evidence` of the feature
   (test phrases, doc sections, commit subjects closest by BM25). The LLM decides what is a rule. If the smoke
   test shows rules in business files no use case reaches, add LLM-inferred, hand-editable rule markers
   (`rule-markers.yaml`, checked on the real files like `check_rule`) to prioritize and bound the reading,
   never to filter.
4. One LLM call per feature (the evidence above, numbered excerpts) to state the rules in business terms, cached by content
   hash.
5. Render on the entity and use case pages and per domain; a `get_rules` (or extended `get_feature`) MCP
   tool; count the rules no doc mentions in the debt report.

## Hand measure (deliverable 1)

Read by hand on `delivery_router` (Ruby, 194 lines of code) and linkding (Python/Django, `bookmarks/`).

**delivery_router**: three rules, none declared as such.
* Limit, in a guard: an order is refused when its delivery time is 60 minutes or more (`add_order`, `delivery_too_long`). The `60` is a bare literal; the spec states it ("when can't ship in less than 60 minutes").
* Refusals by lookup: unknown customer or restaurant (`customer_not_found`, `restaurant_not_found`), returned as `[:error, reason]` values rather than raised.
* Computation: the rider chosen is the one with the shortest delivery time; the delivery time is the longer of travel to the restaurant and cooking time, plus travel to the customer. Not a validation: only visible by reading the method bodies.

**linkding**:
* Validation (model fields): URL syntax (`BookmarkURLValidator`, switchable by a setting), minimum 10 for a numeric preference, `unique=True` on API keys and feed tokens, `choices` enums for ten preferences and for asset status (`pending`/`complete`/`failure`); `max_length` on nearly every field (noise: not business rules).
* Validation (form `clean_url`): when editing, a URL already held by another bookmark of the same owner is refused; when creating, the existing bookmark is updated instead. A comment explains it: the rule is stated in plain words next to the code.
* Permission: only a superuser may change the global settings (`PermissionDenied`); the tests state the ownership rules in words (`can_only_edit_own_tags`, `should_only_export_user_bookmarks`, `can_not_delete_tag_of_other_user`), while the code states them through `filter(owner=...)` queries.
* Limit: PDF snapshots above `LD_SNAPSHOT_PDF_MAX_SIZE` (15 MB, from a setting) are refused, checked on the header then while downloading; the preview size has its own setting; the HTTP client refuses blocked addresses.
* Singleton: only one `GlobalSettings` instance may exist (`save` raises).
* State: asset status `pending`/`complete`/`failure`; no transition table, so no transition rule can be stated from the code.

**What this changes in the approach**
* Where rules live, by yield: guard clauses that return or raise a named error, explicit permission checks, `choices` enums, `save`/`clean_*` overrides, size and count limits (literals and settings), then test descriptions that restate ownership and permission rules the code only implies through queries.
* Noise to drop: `max_length`, `blank`, `null`, `db_index`, presence-style validations, and generic framework errors.
* A rule can be a returned value (`[:error, reason]`), not only a raise: the LLM must be told to look at guards that return a named error.
* Rules with no mechanical marker (computation, ownership by query) are only found by the LLM reading the slice or by a test phrase: the test phrases are the best second source.
* Constants and settings with a business name (`delivery_too_long`, `LD_SNAPSHOT_PDF_MAX_SIZE`) are worth stating only when a guard uses them.
* The Rails test repository was not read in this measure; its natures (validations, policies, state machines) are the ones ADR 0021 lists and stay to check at the smoke test.

## Smoke test (deliverable 8)

`just smoke ../delivery_router --rules`, local Ollama, `qwen3.6:35b-a3b` (the third run below is the
same code with a prompt sentence tried and then reverted).

* First run: 21 rules, 9 calls, 1m41s; rerun reused every feature, no call. Every rule had a confidence of
  0.4: the extracts of tests and docs only existed with `brief.evidence = true` (off by default). Fixed:
  `UseCaseContext.rule_evidence`, filled when `[pipeline] rules` is on, apart from the evidence of the
  features and use cases (their prompts do not change).
* With the fix: `SMOKE TEST CONCLUSIVE` (no warning, rerun a no-op with no call, report produced). 28 rules,
  5 calls, 1m54s, 8 of them backed by a test too (0.7). The expected rules are there: unknown customer or
  restaurant refused, the 60-minute limit, the fastest rider chosen, the delivery time computation.
* Not solved, to measure on a larger repository before acting: (1) the same rule is stated by several
  features (the 60-minute limit twice, the delivery time formula three times): no deduplication across
  features; (2) plumbing is stated as rules ("an order is stored by the customer id", "a rider is
  retrieved by id"); one prompt sentence asking to leave storage and retrieval out did not change the
  count (30 rules) on this small repository, so it was not kept; (3) the debt report lists all 28 rules
  as mentioned in no doc, true here (the only doc is a README that does not state them), so the
  criterion is not validated against a project with real docs.
* An earlier run showed a `WARN` for an invalid chunk boundary regex written by the roles LLM; it comes
  from the roles pass, not from this one.

**linkding, DeepSeek (`deepseek-chat`), `--no-confidence`, concurrency 4, 2026-10-12**
* 66 features, 649 rules (about 10 per feature) in 1m54s, 75 calls: 500k tokens in, 76k out. Use cases:
  437k in, 133k out; the whole run 1.18M in, 242k out, 8m03s. The rules pass is about 42% of the input
  tokens of a run. In money, with the prices I remember for `deepseek-chat` (not recorded by the
  project; to check), about 0.2 $ for the pass and 0.6 $ for the run, more than the 0.05 to 0.10 $
  estimated before the run.
* 199 rules (31%) backed by a test or a doc too (185 at 0.7, 14 at 0.85); 450 by code alone (0.4). By
  domain: 509 in the main one, 70 in `testing-and-quality`, 44 administration, 26 documentation.
* Defect found and fixed: the server refused the strict JSON schema, and 10 answers did not parse
  (`docs`, `tests` or `entities` given as objects or as a bare id where a string was asked), one feature
  was lost. The raw answer types are now lenient (an element of the wrong shape costs the element). The
  rerun asked the lost feature alone (1 call) and got 4 more rules (653).
* The rerun made no other call. The debt report lists 275 of 653 rules as mentioned in no existing doc
  (the project has docs, so the criterion is not trivial; not checked by hand).
* Quality, on a sample of 25: the permission and ownership rules are right and useful ("a bookmark can
  only be deleted by its owner", tags unique per user ignoring case, a duplicate URL refused when
  editing). Noise: user interface behaviour ("pressing n starts adding a bookmark"), rules of the
  end-to-end test features (70, "the browser session is authenticated as the test user"), and kinds that
  fall back on `computation` (201) and `state_transition` (177). Duplicates across features are fewer
  than feared: 38 near-identical pairs among 649.
* One test of `chunk_check` failed once in eight runs of the suite and not again in six more; not
  reproduced, probably a flake of the log capture in `testing.rs`, not looked into.

# Resources

* ADR `0021`, ADR `0019` (evidence), ADR `0020` (bounding)
* `crates/retrodoc-core/src/model.rs`, `crates/retrodoc-pipeline/src/glossary/`, `actors.rs`, `slices.rs`
* `crates/retrodoc-render/src/markdown.rs`, `crates/retrodoc-mcp/src/tools.rs`

# Hints

* A rule backed by code alone is a hypothesis; one backed by a test phrase too is stronger: reflect it in
  the confidence.
* Avoid restating every `presence` validation as a rule: keep the ones a business reader would ask about.

# Tracking

1. [x] Measure by hand the rules of `delivery_router` and linkding (findings in this issue, nothing from confidential repositories).
2. [x] `BusinessRule` model in `retrodoc-core` (serde round trip).
3. [x] Evidence gathering per feature (slices, glossary entities, `Evidence`), no hard-coded selectors.
4. [x] Rules pass: LLM statement in business terms, validated citations, confidence from the evidence, `rules.yaml`, fingerprint, `[pipeline] rules` (off by default), pass name.
5. [x] Rendering on entity and use case pages, per domain, `llms.txt`.
6. [x] MCP: rules in `get_feature`, `get_use_case`, `get_domain` and `search_docs` (no new tool).
7. [x] Debt report: count the rules no existing doc mentions.
8. [x] Smoke test on `delivery_router` (local Ollama) and run on linkding with DeepSeek: figures in Findings.

## Decisions

* Manual measure on `delivery_router` and linkding (checkout in `/tmp/lbf/linkding`).
* The pass runs after the use cases and before the diagrams; result in `rules.yaml`, fingerprint per feature.
* No hard-coded rule selectors (not tied to a language or framework): the LLM reads the slices with the evidence and cites numbered lines, checked mechanically (ADR 0026). Inferred markers only if the smoke test shows a coverage gap. `[pipeline] rules = false` by default until validated on a real repository.
* MCP: no new tool; rules appear in the existing ones and in `search_docs`.
* Rule slug is `<feature-slug>-<n>` for now; a hash of the statement if the number shifting between runs is a problem.
