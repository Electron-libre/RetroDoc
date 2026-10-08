# Write a product brief from the non-code evidence before the surface passes

# Goal

Give every pass a global frame: what the application does, for whom, its main business objects and
capabilities. Today no pass asks it, and the evidence that answers it best (docs, commit messages, tests,
schema, i18n) is barely read. Deliver the signal collection and the product brief of ADR 0019, saved in an
editable `.retrodoc/cache/product.yaml`, and feed the brief to the later prompts.

# Findings (code read on 2026-10-07)

* Existing docs reach a single prompt, the domain clustering, as their first non-empty line only
  (`crates/retrodoc-pipeline/src/domains/prompt.rs`); the default `existing_docs_paths` is `docs` and
  `README.md`.
* `FileHistory` keeps commit count, authors and dates; no commit message is read
  (`crates/retrodoc-ingest/src/git_history.rs`).
* Test descriptions are extracted (`glossary/mod.rs`, `test_vocabulary`) and stored, but no LLM pass reads
  them.
* Schema, migrations, i18n files, `.feature` files and changelogs are not read as sources.

# Approach

1. Signal collection in `retrodoc-ingest`, no LLM: doc sections (split on headings), commit subjects
   (read in the existing revwalk, deduplicated, Conventional Commits types kept), manifest metadata, schema
   and migration table names, i18n keys and values, test descriptions, `.feature` scenarios, the tree two
   levels deep. Each signal keeps its origin. Measure the volume on the Rails test repository.
2. A bounded sample of those signals (budget in characters, the most informative first) for one to three
   LLM calls that write the brief: purpose, users and actors, main objects, candidate capabilities,
   external systems, open questions, each claim citing signals. Reused while its input fingerprint is
   unchanged; a hand edit is kept (as `roles.yaml`).
3. A `retrodoc brief [--force]` command to run and inspect it alone, like `roles`.
4. Inject the brief at the head of the prompts of roles, glossary, entry points, actors, domains, features
   and use cases; include its fingerprint in theirs.
5. Retrieval per unit: share the BM25 of `retrodoc-mcp` with the pipeline, and give the features and use
   cases passes the few doc sections, commits and test phrases that match their unit.
6. Compare with `issues/quality_benchmark.md` before and after.

# Resources

* ADR `0019`, ADR `0008`, ADR `0001` (crate direction, for where the BM25 goes)
* `crates/retrodoc-ingest/src/` (`existing_docs.rs`, `git_history.rs`, `walker.rs`)
* `crates/retrodoc-pipeline/src/roles/` (the editable-artifact pattern), `surface.rs`, `domains/prompt.rs`
* `crates/retrodoc-pipeline/src/bm25.rs` (moved from `retrodoc-mcp`)
* `issues/locate_business_files.md` (takes the brief as input)

# Hints

* Keep the brief prompt within a 32k-context local model: the signals are sampled, never pasted whole.
* The history must stay one revwalk (`CLAUDE.md`: no `git log` per file).
* Merge and bot commits (dependency bumps) are noise for the brief.

# Tracking

1. [x] Signals: doc sections (`Signal` type, headings split, root `*.md` included)
2. [x] Signals: commit subjects in the existing revwalk (no merges or bots, deduplicated, Conventional Commits first)
3. [x] Signals, in three commits (stack-agnostic: no file location or format is hard-coded for one framework):
   * 3a. [x] manifests, tree two levels deep, `.feature`, test descriptions
   * 3b. [x] schema, migrations and i18n readers driven by a `SourceMap` (rules `kind + glob + format`), with a deterministic content-sniffing fallback; `signals` wired into `IngestResult`/`collect`; tested on several stacks (Rails, Django/Alembic, Flyway, i18next, gettext, Java properties)
   * 3c. [x] LLM inference of the `SourceMap` (`signal-sources.yaml`, hand-editable, rules checked against the real files), before the brief
4. [x] Brief: bounded sample, LLM pass, `product.yaml` (fingerprint reuse, hand edit kept, validated citations)
5. [x] `retrodoc brief [--force]` command, with a signal-volume diagnostic; measure on the Rails test repository
6. [x] Inject the brief in the prompts and fingerprints, in two commits:
   * 6a. [x] `generate` writes or reuses the brief first; roles, glossary, entry points, actors read it
   * 6b. [x] domains, features, use cases read it
7. [x] BM25 retrieval per unit, in two commits:
   * 7a. [x] move the BM25 to `retrodoc-pipeline`, `retrodoc-mcp` uses it from there (structural, tests unchanged)
   * 7b. [x] index of the signals, retrieval per unit for features and use cases (budget, fingerprint)
8. [x] Benchmark before and after, final docs and ADR 0019 update: no proven gain, the per-unit evidence is off by default (`brief.evidence`), the new tries are in `issues/brief_injection_experiments.md`

## Decisions

* After the benchmarks (8): the per-unit evidence (7b) is off by default, `[brief] evidence = true` switches it on; the brief stays on. The code and tests of 7b stay for the next experiments (`issues/brief_injection_experiments.md`, to do when the product is more stable). The extracts are shown without their origin: with it, the model cited `spec/` files as source files of the steps.

* Retrieval per unit (7b): `brief::Evidence` indexes doc sections, test descriptions/scenarios and commit subjects (schema, migrations, i18n, manifests and tree stay the brief's); query = names, description and file stems of the unit; budget 1,200 (docs) + 900 (tests) + 900 (commits) characters, 400 per extract; the extracts are in the unit's fingerprint (a related new commit redoes that unit only). `Bm25::search_any` drops the half-of-the-words rule, which a long query would never meet.

* One commit per deliverable; the BM25 retrieval (7) comes last and may become its own issue if it grows.
* The BM25 moves from `retrodoc-mcp` to `retrodoc-pipeline` (no new crate, ADR 0001 direction kept).
* The brief runs first in `generate`, before roles, with its own signals (tree, manifests, docs, commits...).
* The brief uses the same model as the other passes (no wait for `issues/model_per_pass.md`).
* RetroDoc targets any stack (user's repeated requirement): the readers know formats, never framework locations. A `SourceMap` says which files to read in which format, inferred by content sniffing first (3b) then by one LLM call (3c), saved and hand-editable like `roles.yaml`.
* `generate` uses a saved brief as it is (edited or not) and only writes one when there is none; `retrodoc brief [--force]` refreshes it. Otherwise any new test or doc would change the sample and invalidate every pass. `roles.yaml` is not invalidated by a new brief (only the passes with a fingerprint are).
* To check on the benchmark (8): in one smoke run on a small Ruby gem, the use cases named an actor absent from the known actors twice with the brief, never without it (one run each, non-deterministic model); the brief mentions that actor in prose.
* Root-level `*.md` files (README, CHANGELOG...) are read as doc signals; the default `existing_docs_paths` is unchanged.

## Measures (deliverable 5)

On a throwaway full clone of the Rails test repository (about 7,500 commits), `retrodoc brief --signals`:

| Kind | Signals | Characters |
|---|---|---|
| Commit subjects | 7,543 | 339,000 |
| Test descriptions | 521 | 164,000 |
| Translations (one language) | 43 | 30,000 |
| Documentation sections | 24 | 8,000 |
| Schema | 1 | 15,000 |
| Others (manifests, tree, migrations) | 4 | 4,300 |
| Total | 8,136 | 560,000 |

The sample sent to the LLM is 18,600 characters (budget 20,000, headers included): the signals are 30 times
bigger than what a 32k-context model can read, so the sampling is needed, not a detail.

First real run (local `qwen3.6:35b-a3b`, 32k context): the sources pass costs 2 calls (25,600 tokens in, a rule
for an empty schema file was sent back, then dropped), the brief 1 call (5,500 tokens in, 0.9 to 1k out, about 40 s).
The brief reads as a correct business summary of the application (purpose, four kinds of users, central objects,
two external services). What did not work:

* The purpose was cut mid-sentence by the 300-character cap: raised to 600 and cut at a word.
* The model answers `capabilities` as bare strings in 3 runs of 3 even when told to give objects with signals,
  so they all read "unsupported". Handled as designed (kept and flagged); to compare with another model in
  deliverable 8, and a mechanical grounding of uncited claims is a possible follow-up.
* Citations lean on test descriptions and translation files, which are the biggest kinds of evidence after the
  commits; docs are few on this repository.

## Measures (deliverable 8)

Benchmarks of 2026-10-08, before (the commit before the signals) and after (this issue), same model on both sides.
The detail and the analysis are in `issues/brief_injection_experiments.md`.

| Setting | Result |
|---|---|
| Small Ruby gem, local model, 3 runs | use cases fully in business language 41% to 68% (hidden docs) and 65% to 86% (shown); the rest within the spread; input tokens +42% and +22% |
| Small Ruby gem, DeepSeek, 3 runs | nothing beyond the spread (figures already 96% to 100%); 9 and 12 features instead of 6; input tokens +43% and +95% |
| Python web app, local model, 1 run | 42 features instead of 70; judged feature recall 10% instead of 75% |
| Same app, features pass replayed on the same 8 domains | 56 features with nothing, 54 with the brief, 45 with the brief and the evidence |

The "outside the known actors" warning seen once in the smoke test did not come back in 6 runs. The capabilities
of the brief were cited in the two briefs read after the benchmark. No gain of the brief is proven; its effect
is neutral on coverage and positive, but small, on business language with the local model.
