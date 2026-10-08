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
