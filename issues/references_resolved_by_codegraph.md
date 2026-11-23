---
priority: low
why: 'a measure with no LLM that no milestone needs; it decides whether resolved references deserve work at all'
---
# Measure whether the references codegraph resolves beat the lexical slices and the entry points

# Goal
RetroDoc links files by name: slices follow referenced identifiers as text, `ranking.rs` counts the files mentioning
a file's name, contexts and routes are placed by the names their files write. A call resolved by its name alone was
measured as noise (`issues/done/parsed_references_for_ranking_and_slices.md`). codegraph (MIT, open source) claims
to resolve calls, imports and inheritance across files to their definition, and to read the routes of 20+ web
frameworks with no LLM. Expected: a measure, with no LLM and no paid call, of whether its references and routes
would change RetroDoc's slices and entry points for the better; then a decision to drop, plug in or port it.

# Findings
* Measured 2026-10-28 (`parsed_references_for_ranking_and_slices`, deliverable 1): slices from calls resolved by
  name differ from the lexical ones by as much noise as they remove; the noise comes from method names called on a
  receiver whose type the parser does not know, resolved to any file defining a method of that name. Resolving only
  to classes, modules and functions makes the slices almost the lexical ones. The `parsed_slices` example and
  `Follow` stayed as the measuring tool.
* Read from the codegraph repository on 2026-11-23 (its README, `package.json`, `codegraph-kernel/Cargo.toml`,
  `src/lib.rs`, `src/db/schema.sql` and the release page, nothing run): v1.6.4 of 2026-11-20.
  * Its Rust kernel (`codegraph-kernel`, `publish = false`) is a Node addon (`cdylib`, napi exports only) that only
    parses and extracts each file, which `structure.rs` already does. The resolution across files, the framework
    routes and the SQLite store are in TypeScript.
  * Ways in for a Rust program: the CLI as a subprocess (`--json` on `query`, `callers`, `callees`, `impact`,
    `files`, `status`; `context --format json`; not confirmed for `explore`), its SQLite file
    `.codegraph/codegraph.db` (tables `nodes` and `edges`, edge kinds calls, imports, extends, implements,
    references; versioned by migrations, not stated stable), or its MCP server (`codegraph serve --mcp`).
  * The release archives (about 60 MB each) bundle their own Node runtime; the npm package needs Node 20 or later.
  * Anonymous telemetry is on by default (`CODEGRAPH_TELEMETRY=0` or `DO_NOT_TRACK=1` turns it off). Its figures
    (cross-file coverage 74 to 100% per language, fewer tokens for coding agents) are its own, not checked here.

# Approach
Measure first, with no LLM, on the frozen inputs of the benchmark repositories (linkding, Lobsters) and the default
smoke repository; telemetry off; the confidential repositories only if the user agrees.

1. Index each repository with the codegraph CLI and export its edges (CLI `--json`, else a read of its SQLite file
   by a throwaway script, not by RetroDoc).
2. Slices: add a fourth slice to the comparison of `parsed_slices` (lexical, mixed, parsed alone, codegraph), per
   entry-point file of the saved runs, same bounds (depth 2, 8 files): files in common and on one side, read on a
   sample as the 2026-10-28 measure was, method calls first, where the name resolution failed.
3. Entry points: compare the routes codegraph finds with the `entry-points.yaml` of the saved runs (found by both,
   by one side only).
4. On a gain a reader can see, propose, for the user to choose: (a) an optional accelerator that calls the codegraph
   CLI when it is installed, with today's path as the fallback (ADR 0036); (b) a port to Rust of its resolution
   (import-aware, receiver types) into `structure.rs`, keeping one binary with no Node and no telemetry; (c) codegraph
   as a tool of the `ask` and `chat` agent only (callers, callees, impact). With no gain, record why and close.

# Resources
* `issues/done/parsed_references_for_ranking_and_slices.md` (its results and decisions)
* ADR `0022`, ADR `0027`, ADR `0036`
* `crates/retrodoc-ingest/src/structure.rs`, `crates/retrodoc-pipeline/src/slices.rs`, `ranking.rs`
* the `parsed_slices` example of `retrodoc-bench` (`docs/dev/commands.md`)
* `docs/crates/pipeline.md` (`contexts/`, `route_owner.rs`)
* the codegraph repository: `https://github.com/colbymchenry/codegraph`

# Hints
* The ledger of measures lists the saved runs and frozen inputs to reuse; append the measure there.
* Never let codegraph's telemetry run on a confidential repository.
* Its schema is internal: a measure may read it, RetroDoc code must not depend on it.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-23

## Understanding
Measure, with no LLM, whether codegraph's resolved edges give slices and entry points a reader judges better than
today's lexical ones; then record the decision (drop, accelerator, port, agent tool). Out of scope: any code of
RetroDoc's pipeline depending on codegraph; the choice among the follow-ups, left to the stop after deliverable 1.

## Plan
1. [x] Measure: codegraph (release archive, its own Node; telemetry off) indexes a copy of each checkout of the
   saved runs (linkding `rules-ld-judged`, lobsters `second_application_in_benchmark-d8d8c92-1`, delivery_router
   `rules-dr-judged`); its edges and route nodes are exported from its SQLite file to a file-level TSV by a
   documented `sqlite3` query (no RetroDoc dependency); `parsed_slices --codegraph <tsv>` adds a fourth slice
   (codegraph edges, same depth 2 and 8 files, rank by edge kind then path) and a count of the entry-point files
   of `entry-points.yaml` its routes reach (both, ours only, codegraph only); verified by unit tests of the TSV
   reading, the follow and the route comparison; results here and in the ledger.
   **Stop after it**: the user judges, on a sample of differing slices, whether codegraph's are better (decides 2).
   (autonomous)
2. [x] Decision: the outcome written in `docs/findings.md` (dated) and, with a gain, the follow-up issue of the
   option the user picks (accelerator, port into `structure.rs`, agent tool); verified by `check-issue` on it.
   (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 7ad832a1eab80d398681b0cbacf1bb9438c46f39
- Install: the v1.6.4 release archive (its own Node; the machine's Node 26 is outside its `engines`), in the
  session's scratch directory, telemetry off (user, 2026-11-23).
- Repositories: linkding, lobsters, delivery_router only; not the confidential Rails one (user, 2026-11-23).
- After the measure: port the resolution into `structure.rs`, mixed with the lexical fallback and measured first
  (option a, user, 2026-11-23): `issues/resolved_references_in_structure.md`; no dependency on codegraph.

## Results of deliverable 1

codegraph 1.6.4 (release archive, telemetry off) on copies of the checkouts of the saved runs, indexed in under 2 s
each (linkding 4,334 nodes and 12,230 edges; lobsters 3,303 and 4,947; delivery_router 68 and 85); edges between two
files, all kinds but `contains` (calls, imports, instantiates, references, extends, decorates). `parsed_slices
--codegraph --detail`, depth 2 and 8 files, from each entry-point file alone; columns as in
`parsed_references_for_ranking_and_slices`, then the codegraph slice.

| run | entry-point files | lexical | mixed | codegraph | lexical vs mixed | lexical vs codegraph |
|---|---|---|---|---|---|---|
| linkding | 22 | 7.6 | 7.6 | 3.8 | 140 / 28 / 28 | 35 / 133 / 49 |
| lobsters | 95 | 7.7 | 7.7 | 5.7 | 499 / 230 / 237 | 241 / 488 / 302 |
| delivery_router | 2 | 8.0 | 8.0 | 6.0 | 16 / 0 / 0 | 12 / 4 / 0 |

* Where it links, codegraph's slice holds no method-name noise in the sample read (`views/bookmarks.py`,
  `api/routes.py` of linkding; `stories_controller.rb`, `comments_controller.rb`, `notify_comment_job.rb` of
  lobsters): no `lib/time_series.rb`, old migration, `app/javascript/user.js` nor `config/routes.rb`; it brings the
  models, queries and collaborators the code uses (`models.py`, `queries.py`, `hidden_story.rb`, `saved_story.rb`,
  `comment_vote_hydrator.rb`, `notify_comment_job.rb`) and the parent classes (`application_controller.rb`). Noise
  left: a console initializer (`config/initializers/console_methods.rb`), `extras/routes.rb` as a callee.
* It links nothing for 24 of the 119 entry-point files: the Django management commands (7 of linkding; they call
  each other by `call_command` strings), two small views, every `.rake` task and extensionless script of lobsters
  (not parsed as Ruby), `application_job.rb` and `application_mailbox.rb`. The lexical follow finds 7 or 8 files for
  them. Slices are half the size on linkding for that reason.
* Routes: codegraph knows HTTP routes only (linkding 53, lobsters 199, delivery_router none). On lobsters its 42 route
  files are all entry-point files already; the 53 others are jobs, mailers, mailboxes, models, rake tasks and
  scripts, which it does not see as entry points. On linkding, 11 files in common; it adds `views/assets.py` and
  `views/auth.py` (real routes the saved run missed) and four files that declare or test routes (`urls.py`,
  `admin.py`, `api/serializers.py`, a test); it misses `views/reload.py` and links one route to the wrong file
  (`bookmarks/archived` to the `archived` method of `api/routes.py`, not the view).
* Artifacts: `references_resolved_by_codegraph-7ad832a-1/` of the measure folder (`detail.md`, the TSV exports per
  run).

## Summary of deliverable 1

done: `parsed_slices --codegraph` adds the slice along codegraph's edges and the comparison of its routes with the
entry-point files; measured on the three saved runs.

| check | result |
|---|---|
| tests | `parsed_slices` 5 pass (3 new) |
| lint | clippy pedantic clean |
| specs | none (a measure) |
| docs | `docs/dev/commands.md`, doc of the example |
| code review | 1 finding, fixed |

red: none (an example of `retrodoc-bench`, no requirement); the 3 new tests failed to compile before the code
(`Graph`, `route_files`, `route_row` missing).

Code review: 1 findings
* nit: with `--codegraph`, a run dir with no `edges.tsv` printed a row two cells short; fixed (`- | -`).

not verified: the quality of the codegraph slices beyond the five entry-point files read; decisions needed: the
stop below.

## Summary of deliverable 2

done: `docs/findings.md` §13 (the measure and the decision) and the follow-up issue
`issues/resolved_references_in_structure.md` (low, after this one).

| check | result |
|---|---|
| tests | none (docs) |
| check-issue | clean |
| docs | `docs/findings.md` |
| code review | 1 finding, fixed |

red: none (docs only)

Code review: 1 findings
* nit: the links to this issue pointed at `issues/done/` before its close; now its current path, which `close-issue`
  rewrites.
