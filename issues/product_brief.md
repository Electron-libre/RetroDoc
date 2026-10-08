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
* `crates/retrodoc-mcp/src/bm25.rs`
* `issues/locate_business_files.md` (takes the brief as input)

# Hints

* Keep the brief prompt within a 32k-context local model: the signals are sampled, never pasted whole.
* The history must stay one revwalk (`CLAUDE.md`: no `git log` per file).
* Merge and bot commits (dependency bumps) are noise for the brief.

# Tracking

1. [x] Signals: doc sections (`Signal` type, headings split, root `*.md` included)
2. [x] Signals: commit subjects in the existing revwalk (no merges or bots, deduplicated, Conventional Commits first)
3. [ ] Signals, in two commits:
   * 3a. [x] manifests, tree two levels deep, `.feature`, test descriptions
   * 3b. [ ] schema and migrations, i18n, and `signals` wired into `IngestResult`/`run()`
4. [ ] Brief: bounded sample, LLM pass, `product.yaml` (fingerprint reuse, hand edit kept, validated citations)
5. [ ] `retrodoc brief [--force]` command, with a signal-volume diagnostic; measure on the Rails test repository
6. [ ] Inject the brief in the prompts and fingerprints: 6a roles, glossary, entry points, actors; 6b domains, features, use cases
7. [ ] Move the BM25 to `retrodoc-pipeline` and retrieve per unit for features and use cases
8. [ ] Benchmark before and after (ask before running: real cost), final docs and ADR 0019 update

## Decisions

* One commit per deliverable; the BM25 retrieval (7) comes last and may become its own issue if it grows.
* The BM25 moves from `retrodoc-mcp` to `retrodoc-pipeline` (no new crate, ADR 0001 direction kept).
* The brief runs first in `generate`, before roles, with its own signals (tree, manifests, docs, commits...).
* The brief uses the same model as the other passes (no wait for `issues/model_per_pass.md`).
* Root-level `*.md` files (README, CHANGELOG...) are read as doc signals; the default `existing_docs_paths` is unchanged.
