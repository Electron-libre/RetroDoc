# Locate where the business lives with a dedicated LLM pass, not through a `model` role

# Goal

Replace the guess "the business lives in the files classed `model`" with a pass whose only job is to find
where the business is in this stack. Many applications have no identifiable model layer (plain domain
objects under `lib/`, services holding the rules, a functional core, a hexagonal layout) and still do
business. Knowing which files and directories carry it is the base of everything after: the glossary, the
entry points, the actors, and the domain clustering all start from the files they are given. Today they
get them from the roles pass, which sees the file tree and the manifests and nothing else.

# Findings (smoke runs of 2026-10-07 on `delivery_router`, local Ollama, `qwen3.6:35b-a3b`)

* Ten runs of the roles pass on the same small Ruby library (9 files under `lib/`): five give a `model`
  rule that matches files, five leave no `model` file. In three of the five empty runs the whole `lib/**/*.rb`
  tree is classed `entrypoint` (the public API of a library), in the other two `logic`. See
  `issues/roles_no_model_files.md`.
* The fallback delivered by that issue (read the `logic` then the `entrypoint` files, 30 at most, in path
  order) gets the entities back on this repository (3 to 4 entities, in one call), but it is a patch on the
  same assumption: it picks files by path, not by how likely they are to hold the business.
* Roles are a layer vocabulary (`entrypoint`, `model`, `logic`, `view`, `infra`, ...). `CLAUDE.md` and ADR
  0008 want domains named from business concepts, never layers; the input is still chosen by layer.
* Measure of 2026-10-08 (deliverable 1; `retrodoc roles --force` five times per repository, local Ollama,
  `qwen3.6:35b-a3b`, no saved brief; the business files were listed by hand beforehand):
  * `delivery_router` (8 business files out of 9 under `lib/`): `model` matched them in 2 runs of 5 (8/8);
    in the 3 others the files were `entrypoint` (2 runs) or `logic` (1 run), so the glossary relied on the
    ADR 0018 fallback.
  * linkding (a Django app, 13 hand-listed business files: `models.py`, `queries.py`, `validators.py`,
    8 services): `models.py` was classed `model` in 2 runs of 5 (recall 1/13); in the 3 others the rule
    pointed to a `bookmarks/models/` directory that does not exist. The services were `logic` in every run
    but `queries.py` and `validators.py` never were. `bookmarks/migrations/**` was `model` in all 5 runs
    (about 54 files): the glossary reads the migrations and, in 3 runs of 5, not the models file.
  * So the `model` role has a recall of 0.0 to 1.0 on a small library and at most 1/13 on a real app, with
    the migrations as its main noise. The decision that matters is the location of the business, and the
    role vocabulary does not carry it.
* Measure of 2026-10-08 after the pass exists (deliverable 5; `retrodoc business-files --force` five times per
  repository, same hand-listed business files, same local model):
  * `delivery_router`: 8, 7, 8, 8, 8 of 8 files found with a first prompt, 8, 8, 8, 5, 8 with the final one
    (the `model` role: 8, 0, 0, 8, 0).
  * linkding, first prompt: 8, 11, 11, 10, 13 of 13 (mean 10.6), 20 to 73 files once the folders are
    expanded, precision against the hand list 0.33; one run answered the whole `bookmarks/` folder, another
    the `tests_e2e/` folder. Final prompt (presentation layers, tests and the root folder excluded): 12, 9, 7, 8,
    6 of 13 (mean 8.4), 10 to 47 files, precision 0.48. The `model` role: 1, 1, 0, 0, 0. The final prompt was
    written after seeing the first runs on this same repository, so its figures are not an independent test;
    the views, serializers and API routes still appear in most runs.
  * Smoke test on `delivery_router` (`generate` twice, local model): it ended, the second run wrote 0 files and
    called no LLM. It exposed a bug: the tree shape counted the docs written by the first run, so the second run
    asked again; the shape now counts source files only. Warnings left in the first run come from the roles
    chunk-boundary check and one use-case answer that needed a retry, not from this pass (which only warns when it
    drops a path that does not exist).

# Approach

First measure, as in the previous issue: on two or three repositories of different shapes (a Rails app with
a `models/` folder, a small library of plain classes, one with a service-oriented or hexagonal layout), list
by hand the files that hold the business, then compare with what the roles pass classes `model`.

Then, candidates (can be combined):

1. A pass, before the glossary, that asks the LLM where the business is. Input: the file tree, the
   manifests, the stack from the roles pass, and cheap evidence per directory or file (names, the first
   lines or the signatures of a sample, the test vocabulary, doc titles, git churn). Output: a ranked,
   bounded list of files and directories likely to hold business rules and data, each with a short reason,
   saved in an editable file next to `roles.yaml`.
2. Feed that list to the glossary, the entry points and the actors instead of (or in addition to) the
   `model` and `entrypoint` roles; the fallback of ADR 0018 would then be dropped or kept only as a safety net.
3. Decide how it relates to the roles pass: replace the `model` role, complement it, or keep roles for the
   mechanical classification (tests, config, docs) and leave the business location to the new pass.
4. Cost: one call over a bounded input per run, cached like `roles.yaml`; check the prompt size on a large
   repository (the tree alone may not fit).
5. Take the product brief of ADR 0019 (`issues/done/product_brief.md`) as input: its main business objects and
   capabilities say what to look for, so this pass comes after it.

# Resources

* `crates/retrodoc-pipeline/src/roles/` (prompt, `RoleRules::classify`, `RoleMap::files_with`)
* `crates/retrodoc-pipeline/src/glossary/`, `entry_points/`, `actors.rs`, `surface.rs`
* ADR `0008` (surface extraction), ADR `0018` (glossary fallback), `issues/roles_no_model_files.md`
* `PLAN.md` §7.1 (phase 7, business-level documentation)
* ADRs `0019` (product brief first) and `0020` (cover behaviours, not files), `issues/done/product_brief.md`

# Hints

* The model answers differently each time: judge on several runs and on repositories of different shapes,
  not on `delivery_router` alone (it is tiny and every file is business).
* Don't edit `roles.yaml` by hand to make a run pass (smoke test rule).
* The pass must not become one more call per file: bound its input, as the file budget does.

# Tracking

1. [x] Measure the current `model` role: hand-listed business files vs the roles pass, 5 runs each on `delivery_router` and linkding (recall, precision, variance); record the figures in Findings.
2. [x] `business_files/` module: `BusinessMap` (path or directory, reason, rank), prompt over a bounded input (tree, stack, brief, cheap evidence), hallucinated paths dropped, editable `business-files.yaml` with the `roles.yaml` reuse rules. Tested with `FakeLlm`, including the prompt size bound on a large tree.
3. [x] `retrodoc business-files [--force]` and wiring in `generate` (after the brief, before the glossary; token recap). Tested: order, no LLM call on the second run.
4. [x] The glossary reads the `BusinessMap` (directories expanded to bounded `Source` files) instead of the `model` role; the ADR 0018 fallback stays for now. Tests: existing glossary tests adapted, a repo with no `model` role.
5. [x] Validate on real repositories: smoke test on `delivery_router`, quality benchmark on linkding (3 runs, before/after). Success: better recall of business files than the `model` role and a non-empty glossary on every run; otherwise rework the evidence before going on.
6. [ ] Remove the ADR 0018 fallback, write ADR 0025 (supersedes 0018), update `CLAUDE.md`, `PLAN.md`, `docs/ARCHITECTURE.md`; offer to close the issue.

## Decisions

* Repositories: `delivery_router` and linkding (public clone in `/tmp/retrodoc-benchmark-linkding`).
* The new pass complements `roles` (which keeps the mechanical classification); `model` stops being the glossary's source.
* The entry points keep the `entrypoint` role for now; revisit only if the measure shows a gap. The actors pass reads authorization code, not roles, so it is unaffected.
* No quality benchmark on linkding (cost and time): deliverable 5 rests on the recall measure above and the smoke test; the glossary of linkding was read once with the first prompt (37 entities, about half of them presentation classes).
* The ADR 0018 fallback is removed only after real-repository validation (deliverable 6).
