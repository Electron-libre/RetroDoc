# Read definitions, signatures and references with tree-sitter

# Goal

Replace the LLM-negotiated chunk boundaries and the lexical guesses of symbols and references with a real
parse, for a first set of languages (ADR 0022). Expected: fewer calls and warnings at chunking, smaller
prompts (signatures instead of bodies where names suffice), and a deterministic structure map that the
behaviour-based pipeline (`issues/behaviour_coverage_domains.md`) can rank and slice on.

# Findings

* Chunk boundaries are regexes proposed by the roles call, checked by `chunk_check.rs` with up to three fix
  calls (ADR 0013); together with `chunks.rs` that is about 900 lines, and a first-try 0% coverage is one
  of the smoke-run warnings (`issues/use_cases_model_noise_warnings.md`).
* `ranking.rs` counts references by file-name mentions; `slices.rs` follows referenced identifiers
  lexically.

# Approach

1. Check the state of the Rust `tree-sitter` bindings and of the grammars for Ruby, Rust, JavaScript and
   TypeScript, Python (their `tags.scm` queries), and the build time and binary size each adds.
2. A module (in `retrodoc-ingest` or a new crate) that returns, per file, its definitions with line ranges
   and signatures and its references, behind cargo features per language.
3. Use it in `Splitter` for the covered languages; keep the regex path for the others.
4. Feed `ranking.rs` and `slices.rs` with the parsed references; compare the slices on the Rails test
   repository with the lexical ones.
5. Signature-only views for the entry points and glossary prompts; measure the characters saved.

# Resources

* ADR `0022`, ADR `0013`
* `crates/retrodoc-pipeline/src/chunks.rs`, `chunk_check.rs`, `ranking.rs`, `slices.rs`
* ADR `0001` (crate direction)

# Hints

* Dynamic languages (Ruby metaprogramming, Rails conventions) hide references a parser can't see: keep
  the lexical fallback for slices.
* Don't drop the regex path: some repositories use languages without a grammar.
