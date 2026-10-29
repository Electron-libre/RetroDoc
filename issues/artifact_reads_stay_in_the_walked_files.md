# Read only the walked files from a path a saved artifact cites

# Goal
`text::read_file_lossy` now refuses a path that leaves the repository, but a path edited in a saved artifact can
still name a file inside it that the walker skips (a gitignored `.env`, `config/master.key`, `.git/config`); its
content then goes into a prompt sent to the LLM. A path that comes from an artifact should only reach a file the
walker yields, as the MCP only reads cited paths.

# Findings (code review `high` of `issues/done/source_reads_stay_in_the_repo.md`, 2026-10-29)
* The new guard checks that a path is plain and resolves under the root, not that it is a walked file.
* `confidence/checks.rs` (`check_step`) and `confidence/mod.rs` (cited excerpts) read the `source_refs` of the
  saved use cases, a YAML artifact a user or another tool can edit.
* Other callers read paths from the walk or the saved role map; whether the role map is a path in is not checked.

# Approach
1. List the callers whose paths come from an editable artifact rather than from the walk of this run.
2. Candidates (can be combined):
   1. check those paths against the walked file set before reading (the set the run already has);
   2. refuse in `read_file_lossy` a path the walker's ignore rules exclude (costs an ignore match per read).
3. A test per caller: an artifact citing a gitignored file inside the repo is treated as unreadable.

# Resources
* `crates/retrodoc-pipeline/src/text.rs`, `confidence/checks.rs`, `confidence/mod.rs`.
* `crates/retrodoc-ingest/src/walker.rs` (ignore rules, `.git` excluded).
* `crates/retrodoc-mcp/src/source.rs` (`check_cited`).

# Tracking

Plan: validated

## Understanding
A path read from an editable artifact (the `source_refs` of the saved use cases; the `source_paths` of the saved
features, which feed the use cases and rules prompts, inputs, groundings and fingerprints) is read only when it is a
file of this run's walk; otherwise it is unreadable, as a missing file is. No spec: an internal guard, like
`issues/done/source_reads_stay_in_the_repo.md`; no behaviour changes for a walked path. Means: option 2.1, the set is
taken from the full walk, before the file budget. The other callers read walked paths (ingest files, `CodeIndex`,
entry points filtered by the walked sources, role map applied to `ingest.files`, fixed manifest names): unchanged.

## Plan
1. [x] `WalkedFiles` (the relative paths of the walk) in `retrodoc-pipeline`, taken by `score_confidence` and
   `mechanical_score`: a cited path outside it is `Broken("cannot read …")` in `check_step` and left out of the
   cited excerpts; `generate`, the benchmark replay and the `confidence_agreement` example pass it; verified by a
   `check_step` test and a `score_confidence` test whose use case cites a gitignored `.env` inside a git repo
   (Broken, not in the prompt); ~250 lines.
2. [x] `UseCaseContext` carries the `WalkedFiles`; `FeatureInput` keeps only the walked files of `feature.source_paths`,
   so the use cases and rules prompts, `RuleInput`, the groundings and the fingerprints never read another; verified
   by a use cases test and a rules test whose feature lists a gitignored file (absent from the prompt sent to the
   fake LLM); ~300 lines.

## Decisions
- Mode: autonomous
- Base commit: 6a059b0e403bef68b76536d1fef8b1093425e5fd
- Set checked: the full walk, before the file budget (1 a).
- Role map: unchanged, it applies to `ingest.files` and gives no path to read (2 a).

## Deliverable 1 summary
- done: `WalkedFiles` (`walked.rs`, the walk taken before the budget in `generate`) is what `score_confidence` and
  `mechanical_score` read cited files through; a file it lacks is `Broken("cannot read …")` and kept out of the
  prompt; the benchmark replay and `confidence_agreement` walk their tree; `ARCHITECTURE.md` §4, `pipeline.md`.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | n/a (no spec) |
| docs | `docs/ARCHITECTURE.md` safety nets, `docs/crates/pipeline.md` confidence row |
| code review | low: no findings |

- red: none (no spec): `check_step` gave `Pass` for a cited gitignored `.env`; the scoring prompt held `SECRET_TOKEN`.
- Code review: no findings
- not verified: a real `generate` run (no paid run for an internal guard).
- decisions needed: none.

## Deliverable 2 summary
- done: `UseCaseContext::walked` (set by `generate`, both paths, and the benchmark replay, which also scores its
  grounding with it); `FeatureInput` keeps only the walked entry files and feature files, so the use cases and
  rules prompts, `RuleInput`, the groundings and the fingerprints read no other; a file gone from the repository is
  now dropped from the input rather than hashed as unreadable (the fingerprint still changes);
  `ARCHITECTURE.md` §4, `pipeline.md` use cases row; `contexts::record_run` taken out of `document` (clippy length).

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | n/a (no spec) |
| docs | `docs/ARCHITECTURE.md` safety nets, `docs/crates/pipeline.md` use cases row |
| code review | low: no findings |

- red: none (no spec): the rules input kept `.env` in its files; the use cases prompt showed the `.env` entry file
  instead of `a.rs`.
- Code review: no findings
- not verified: a real `generate` run.
- decisions needed: none.
