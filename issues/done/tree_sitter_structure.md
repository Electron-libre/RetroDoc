# Read definitions, signatures and references with tree-sitter

# Goal

Replace the LLM-negotiated chunk boundaries and the lexical guesses of symbols and references with a real
parse, for a first set of languages (ADR 0022). Expected: fewer calls and warnings at chunking, smaller
prompts (signatures instead of bodies where names suffice), and a deterministic structure map that the
behaviour-based pipeline (`issues/done/behaviour_coverage_domains.md`) can rank and slice on.

# Findings

* Chunk boundaries are regexes proposed by the roles call, checked by `chunk_check.rs` with up to three fix
  calls (ADR 0013); together with `chunks.rs` that is about 900 lines, and a first-try 0% coverage is one
  of the smoke-run warnings (`issues/done/use_cases_model_noise_warnings.md`).
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

# Tracking

1. [x] Structure module in `retrodoc-ingest`: per file, definitions (name, kind, line range, signature) and references, for Ruby, Rust, JavaScript, TypeScript and Python behind one cargo feature per language (all on by default); build time and binary size measured before and after.
2. [x] Chunking by parse: `Splitter` cuts at definition boundaries for the covered languages; `roles` no longer asks for `chunk_boundaries` for them and `chunk_check` skips them; the regex path stays for the others. Calls and warnings of chunking counted before and after on `delivery_router` and the Rails test repository.
3. [-] (moved to `issues/done/parsed_references_for_ranking_and_slices.md`, user, 2026-10-11) Parsed references in `ranking.rs` and `slices.rs`, lexical fallback kept; slices compared with the lexical ones on the Rails test repository. Decide with the user after the figures of step 2.
4. [x] Signature-only views (built as the opt-in `[pipeline] body_lines`, default unchanged) for the entry points and glossary prompts; characters saved measured. Decide with the user after step 3.
5. [x] Docs (`update-docs`) and an ADR recording the measured results (ADR 0022 is accepted and immutable).

## Decisions

* All five languages of the issue (Ruby, Rust, JavaScript, TypeScript, Python) from the start, features on by default: build cost is not a reason to restrict the set.
* The module lives in `retrodoc-ingest` (repository reading, no LLM, below `retrodoc-pipeline`).
* Order: chunking first (most visible: fewer calls and warnings), then references, then signature views; steps 3 and 4 go on only after the figures of the previous step.
* Measure on `delivery_router` and the Rails test repository with the local Ollama, as for a smoke test; criteria: calls and warnings of chunking, prompt characters, slices compared.
* A saved `roles.yaml` with `chunk_boundaries` stays valid; covered languages simply stop using them.
* Step 1 measured: the grammars and tree-sitter build in about 9 s in release (72 s CPU); the binary size is measured at step 2, once the code is used (the linker drops it before).
* Known: the `tags.scm` queries give calls as references, not types or constants; the TypeScript query is completed with the JavaScript one and classes/enums.
* Step 2 measured (`roles --force`, local qwen3.6): on a small Ruby gem, 1 roles call and no chunk boundary to check (before: the Ruby regex checked at 100%); on the Rails test repository, 1 roles call and no check, no warning, no fix call (before: Ruby 98%, JS/TS regex at 0% on the first try, 1 warning and 1 fix call). Release binary 21.29 MB -> 28.72 MB (+7.4 MB, +35%) with the five grammars; the grammars build in about 9 s. Not measured: the quality of the chunks in a full `generate`; the "file too long" warnings (more than 8 chunks) are unchanged.
* Step 4 upper bound (no LLM, scratch measure, not kept): share of the characters of the files read by each pass that sit in method/function bodies (a signatures-only view could drop at most this): small Ruby gem 51% (entry points) and 53% (glossary); Rails test repository 83% (192 entry point files, 594k chars) and 56% (440 model files, 869k chars). Open question before building it: the entry points pass asks for `outputs` (emails, writes, calls), which live in the bodies, and bodies of model methods hold part of the business rules; only an LLM comparison against `benchmark/<repo>/entry-points.yaml` (`entry_points_replay`) can say what the view costs in quality.
* Step 4 built as an experiment, off by default: `[pipeline] body_lines = N`. Measured with the local qwen3.6, one run per setting, `entry-points` then `glossary` on the Python/Django benchmark repository (linkding) and `entry_points_replay` against its reference: full bodies 18 + 10 calls, 52.8k + 27.9k tokens in; N=5 10 + 6 calls, 30.7k + 16.8k tokens in (-42% and -40%); N=2 8 + 5 calls, 25.2k + 14.0k (-52% and -50%). Entry points: groups found 86% / 86% / 86%, reference files read 84% for the three, entries in reference files 74% / 72% / 87%. Glossary (no reference): 14 entity names with full bodies, 13 with N=5 (lost `FeedToken`, `User`, gained `TagCache`), 11 with N=2 (lost five, gained two). On the small Ruby gem (3.9k characters read) the savings are 1-4% and the entry points found vary from 50% to 100% between runs of the same setting: one sample cannot separate noise from the effect, so the glossary loss is not established.
* Step 4, three runs per setting on linkding (local qwen3.6, same prompts every run so the token counts are exact; supersedes the single-run figures above). Tokens in: entry points 52.8k (full) / 30.7k (N=5, -42%) / 25.2k (N=2, -52%), glossary 27.9k / 16.8k / 14.0k. Entry points, groups of the reference found (3 runs): full 86, 86, 79%; N=5 86, 93, 86%; N=2 86, 79, 86%; reference files read 84% in all nine; entries in reference files full 74, 68, 81%, N=5 72, 61, 47%, N=2 87, 93, 58%: the ranges overlap, no loss measurable above the run-to-run noise. Glossary (no reference), entity names / attributes / associations: full 14, 16, 12 / 118, 132, 103 / 23, 18, 20; N=5 13, 10, 12 / 94, 77, 86 / 18, 12, 22; N=2 11, 14, 16 / 105, 101, 96 / 19, 18, 29. The attributes are lower at N=5 in all three runs (77-94 against 103-132), the entity names overlap. Run-to-run noise is as large as most differences (one full run lists 64 entry points, another 96). Two parse glitches of the model, not tied to the setting as far as seen: a full run cut a file name into a brief excerpt (entry point dropped), and one N=5 glossary run attributed three entities to a file it was not given (dropped).
