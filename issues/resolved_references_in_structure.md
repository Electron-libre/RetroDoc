---
priority: low
depends_on: [references_resolved_by_codegraph]
why: 'a measured gain in slice precision that no milestone needs yet; the port is worth it only if its mix with the lexical fallback holds'
---
# Resolve the references of the parsed files through imports and receivers

# Goal
A call resolved by its bare name brings method-name noise into the slices
(`issues/done/parsed_references_for_ranking_and_slices.md`); codegraph's resolution, which knows imports, parent
classes and receivers, removes it where it links but sees nothing for a fifth of the entry-point files
(`issues/done/references_resolved_by_codegraph.md`, `docs/findings.md` §13). Expected: the same precision from
RetroDoc's own `structure.rs`, with the lexical follow for what it does not resolve, and slices a reader judges
better than today's, with no external tool.

# Findings
* 2026-11-23, linkding, Lobsters, delivery_router, no LLM: lexical vs codegraph slices common / lexical only /
  codegraph only 35 / 133 / 49 and 241 / 488 / 302; mean slice 3.8 and 5.7 files against 7.6 and 7.7. Where
  codegraph links, the five slices read hold no method-name noise and add the models, queries and parent classes.
* codegraph links nothing for 24 of 119 entry-point files: Django commands calling each other by `call_command`
  strings, `.rake` tasks and extensionless scripts (not parsed as Ruby), two small views, two base classes.
* The exports of the measure (`edges.tsv` per run) are kept in `references_resolved_by_codegraph-7ad832a-1/` of the
  measure folder: a reference to compare a port against, file pair by file pair.

# Approach
Measure first, with no LLM, before touching the pipeline.

1. Measure the mix: a `Follow` of `parsed_slices` that takes codegraph's edges first and the lexical follow for the
   files they leave empty, on the same runs; read a sample. Stop if it is not better than the lexical slice.
2. Port, language by language, starting with Python and Ruby (the benchmark repositories): imports resolved to
   files (relative and package paths), a call on `self`, a class constant or a parent class resolved within the
   class's hierarchy, a call on an unknown receiver left unresolved (no bare-name guess); compare its edges with the
   kept `edges.tsv`.
3. Use it in `CodeIndex::slice` behind the same bounds, the lexical follow as fallback (ADR 0036), then consider
   `ranking.rs`.

# Resources
* `issues/done/references_resolved_by_codegraph.md`, `issues/done/parsed_references_for_ranking_and_slices.md`
* `docs/findings.md` §13, ADR `0022`, ADR `0027`, ADR `0036`
* `crates/retrodoc-ingest/src/structure.rs`, `crates/retrodoc-pipeline/src/slices.rs`, `ranking.rs`
* the `parsed_slices` example of `retrodoc-bench` (`--codegraph`)
* codegraph's resolution, MIT: `https://github.com/colbymchenry/codegraph` (`src/`, TypeScript)

# Hints
* Per language the work differs: a per-language resolver must stay behind its grammar feature, and a language with
  no resolver keeps today's path.
* Do not copy codegraph's framework routing: per-framework adapters were rejected (ADR 0036).

# Specs
* `specs/resolved-references.md` (RES, new)

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
Measure the mix (codegraph's edges first, lexical for the files they leave empty); if it beats the lexical slice,
port the resolution of Python and Ruby (imports, parent classes, `self` and class-constant receivers, no bare-name
guess) into a new `resolve` module of `retrodoc-ingest` and cut the passes' slices with it, lexical per file as
fallback. Out of scope: other languages, `ranking.rs`, contexts and routes placement, framework adapters.

## Plan
1. [x] Measure the mix: `parsed_slices --codegraph` adds a fifth slice, codegraph's edges from a file that has
   some, the lexical follow from one that has none (same depth 2, 8 files); measured on the three saved runs, a
   sample of differing slices read with the method of the 2026-11-23 measure; verified by a unit test of the mixed
   follow; results here and in the ledger. **Stop after it**: go on only if the mix beats the lexical slice.
   (autonomous)
2. [x] Python resolver: new module `retrodoc-ingest/src/resolve/` (`mod.rs`, `python.rs`; `structure.rs` is over
   the file-size limit), its own tree-sitter queries behind the `python` feature, giving per file the source files
   it references with a count; verified by tests of the scenarios of RES-3, RES-4, RES-7 to RES-10 on temp
   repositories; makes true RES-1 (Python), RES-3, RES-4, RES-7 (Python), RES-8, RES-9, RES-10. (autonomous)
3. [x] Ruby resolver: `resolve/ruby.rs` behind the `ruby` feature (`require`/`require_relative`, constants by
   nesting, superclass, `include`/`extend`, implicit and `self` calls); verified by tests of the scenarios of RES-5
   to RES-7; makes true RES-1 (Ruby), RES-5, RES-6, RES-7 (Ruby). (autonomous)
4. [ ] Resolved follow and its measure: `Follow::ResolvedThenLexical` in `slices.rs` (per file: resolved references,
   else lexical), a `--resolved` slice in `parsed_slices` compared with the lexical, the mix of deliverable 1 and the
   kept `edges.tsv` file pair by file pair; measured on the three runs, no LLM; verified by the slice tests of RES-2,
   RES-11; makes true RES-2. **Stop after it**: the passes switch only if it holds the mix's gain.
5. [ ] Passes switched: `CodeIndex` built with the resolution and `slice` following it for use cases, contexts,
   the reach of the entry points and the behaviour coverage; verified by RES-H1 to RES-H3 through `generate` with
   a fake LLM (the slice in the prompt) and the existing pass tests; makes true RES-11, the spec `accepted`.
6. [ ] Docs and fixes of the `high` review of the issue (ADR 0027 amended or a new ADR for resolution over the
   bare-name parse, `docs/crates/core-ingest.md`, `pipeline.md`, `docs/findings.md`).

Estimate: ~150 / 800 / 800 / 300 / 200 / 150 changed lines, ~2,400 in all.

## Decisions
- Mode: autonomous
- Base commit: 492c9ffdad512b38bb0efcc5099afa01a4c58ccd
- Judge of the stops 1 and 4: the agent, with the method of 2026-11-23: a sample of 6 differing slices, each file
  on one side only classed used by the code (import, inheritance, known receiver) or noise; the mix wins with less
  noise and no used file lost; the table is shown to the user (user, 2026-11-23).
- No paid measure before the passes switch: the no-LLM measures of deliverables 1 and 4 suffice (user, 2026-11-23).
- Python package roots: the repository root and each directory holding a top-level package, no config read
  (user, 2026-11-23).

## Results of deliverable 1

`parsed_slices --codegraph --detail` at 492c9ff on copies of the three saved runs with the kept exports, depth 2,
8 files; the earlier columns give the figures of 2026-11-23 again. The mix follows codegraph's edges from a file
they link to other source files, the lexical follow from any other file.

| run | entry-point files | lexical | codegraph | mix | lexical vs codegraph | lexical vs mix |
|---|---|---|---|---|---|---|
| linkding | 22 | 7.6 | 3.8 | 7.4 | 35 / 133 / 49 | 97 / 71 / 66 |
| lobsters | 95 | 7.7 | 5.7 | 7.1 | 241 / 488 / 302 | 348 / 381 / 325 |
| delivery_router | 2 | 8.0 | 6.0 | 7.0 | 12 / 4 / 0 | 14 / 2 / 0 |

Sample read (method of the decisions: each file on one side only, used by the code through an import, a parent
class or a known receiver, else noise):

| entry-point file | mix only: used / noise | lexical only: used / noise | read |
|---|---|---|---|
| `bookmarks/views/bookmarks.py` | 3 / 0 (`models.py`, `queries.py`, `utils.py`, all imported) | 0 / 3 (`form.js`, `templatetags/bookmarks.py`, `services/bundles.py`) | mix |
| `bookmarks/api/routes.py` | 3 / 0 (`models.py`, `website_loader.py` imported; `feeds.py` at hop 2) | 0 / 3 (`templatetags/bookmarks.py`, `templatetags/shared.py`, `views/assets.py`: words, not imports) | mix |
| `management/commands/backup.py` | 0 / 2 | 0 / 2 | even: the command imports nothing of the repo, both slices are noise |
| `stories_controller.rb` | 4 / 0 (`application_controller.rb`, `hidden_story.rb`, `keystore.rb`, `saved_story.rb`) | 0 / 4 (`user.js`, `short_id.rb`, `config/routes.rb`, `user.rb` named by `@user` only) | mix |
| `notify_comment_job.rb` | 4 / 1 (`application_job.rb`, `application_mailer.rb`, `keystore.rb`, `extras/routes.rb`; noise `console_methods.rb`) | 0 / 3 (`application.js`, `user.js`, `username.rb`) and 2 models reached by a parameter's name only (`comment.rb`, `story.rb`) | mix, with a loss |
| `lib/tasks/privacy_wipe.rake` | 0 / 0 | 0 / 0 | same: unlinked, lexical fallback |

* Verdict: the mix beats the lexical slice; noise on one side only 12 lexical against 1 mix in the sample, 13
  files used by the code added, no file used through an import, a parent class or a known receiver lost.
* The loss to keep in sight: a job or service handling `comment` and `story` parameters loses their models, reached
  by the lexical follow through the parameter's name; RES-8 loses them too (no type inference).
* Artifacts: `resolved_references_in_structure-492c9ff-1/detail.md` of the measure folder.

## Summary of deliverable 1

done: `parsed_slices --codegraph` adds the mix (codegraph's edges, else the lexical follow, per file); measured on
the three saved runs, sample read.

| check | result |
|---|---|
| tests | `parsed_slices` 6 pass (1 new) |
| lint | `project-check` clean (clippy pedantic) |
| specs | none (a measure) |
| docs | doc of the example, `docs/dev/commands.md` |
| code review | no findings |

red: none (an example of `retrodoc-bench`, no requirement); the new test failed to compile before the code
(`mixed_next`, `slice_by` missing).

Code review: no findings

not verified: the slices of the mix beyond the six files read; decisions needed: the stop of the plan (go on with
the port).

## Summary of deliverable 2

done: `retrodoc_ingest::resolve` (`resolve(repo_root, files)`, `Resolved::targets`) and its Python resolver
(`resolve/python.rs`, behind the `python` feature): imports, bound names followed where used, `self`/`cls` and
class-constant calls looked up in the class then its ancestors, nothing for an unknown receiver.

| check | result |
|---|---|
| tests | `tests/resolve_python.rs` 10 pass (new) |
| lint | clippy pedantic clean, default features, none, `ruby` alone |
| specs | RES-1, RES-3, RES-4, RES-7 to RES-11 traced (Python); `check-specs` clean |
| docs | `docs/crates/core-ingest.md` |
| code review | 1 finding, fixed |

red: RES-H1, RES-1, RES-3, RES-4, RES-7, RES-8, RES-9, RES-10, RES-11: the tests failed to compile before the
code (`unresolved import retrodoc_ingest::resolve`).

Code review: 1 findings
* important: an `__init__.py` at the repository root made the search of the package root loop forever; fixed
  (stop at the root), test `a_repository_with_an_init_at_its_root_still_resolves` (written with the fix, not run
  red).

not verified: the resolution on a real repository (the measure of deliverable 4); decisions needed: none.

## Summary of deliverable 3

done: the Ruby resolver (`resolve/ruby.rs`, behind the `ruby` feature): requires, constants by nesting, reopened
classes, implicit, `self` and constant calls looked up in the class, its parents, then its modules; locals and
parameters are not calls.

| check | result |
|---|---|
| tests | `tests/resolve_ruby.rs` 7 pass (new), `tests/resolve_python.rs` 10 pass |
| lint | clippy pedantic clean, default features, none, `ruby` alone, `python` alone |
| specs | RES-H2, RES-1, RES-5 to RES-8, RES-10 traced (Ruby); `check-specs` clean |
| docs | `docs/crates/core-ingest.md` |
| code review | 2 findings, fixed |

red: RES-H2, RES-1, RES-5, RES-6, RES-7, RES-8, RES-10: 6 tests failed on empty targets before the code
(`left: {} right: {"app/controllers/application_controller.rb", "app/models/story.rb"}`).

Code review: 2 findings
* nit: the names of a multiple assignment (`a, b = pair`) were read as calls on `self`; fixed (declared locals).
* nit: block parameters stayed locals after their block, hiding a later call of that name; fixed (locals restored
  after a block). Test `locals_of_a_multiple_assignment_are_not_calls_and_block_parameters_end_with_their_block`,
  red before the fix (`left: {base.rb, concerns/naming.rb} right: {base.rb, concerns/paging.rb}`).

not verified: the resolution on a real repository (the measure of deliverable 4); decisions needed: none.
