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
3. Candidate detection, bounded by the business files and the use case slices: structural where
   tree-sitter is available (`issues/tree_sitter_structure.md`), lexical patterns otherwise; test phrases
   and doc sections matched as extra evidence.
4. One batched LLM call per group of candidates to state the rules in business terms, cached by content
   hash.
5. Render on the entity and use case pages and per domain; a `get_rules` (or extended `get_feature`) MCP
   tool; count the rules no doc mentions in the debt report.

# Resources

* ADR `0021`, ADR `0019` (evidence), ADR `0020` (bounding)
* `crates/retrodoc-core/src/model.rs`, `crates/retrodoc-pipeline/src/glossary/`, `actors.rs`, `slices.rs`
* `crates/retrodoc-render/src/markdown.rs`, `crates/retrodoc-mcp/src/tools.rs`

# Hints

* A rule backed by code alone is a hypothesis; one backed by a test phrase too is stronger: reflect it in
  the confidence.
* Avoid restating every `presence` validation as a rule: keep the ones a business reader would ask about.
