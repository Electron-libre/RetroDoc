# Share the skeleton of the LLM passes and the context of a run

# Goal
Each pass writes again the same sequence: load the saved artifact, fingerprint the input of a unit, skip
the unchanged ones, call the LLM, ground the answer, save, report progress. The orchestration threads the
same five or six values through every function. Extract what is common so a pass reads as its prompt, its
answer type and its grounding, and a fix of the skeleton (resume, concurrency, fingerprint) is made once.
No behaviour change: the tests, `just check` and a second `generate` writing nothing stay green.

# Findings (project review of 2026-10-12, from the structure; the modules were not all read line by line)
* Twelve `build_*` entry points in `retrodoc-pipeline` with different signatures; 21 `load`/`save`
  functions outside the tests.
* `batched_read.rs` already shares the loop of the glossary and entry points passes
  (`issues/done/refactor_code_hygiene.md`, deliverable 1); the per-unit passes (features, use cases, rules,
  confidence) each have their own loop, with concurrency added to two of them one after the other.
* `business_read/`, `glossary/` and `entry_points/` read the same files with three prompts; since ADR 0028
  the joint reading is the default and the two others read only what it left.
* `generate/mod.rs` is 648 lines; `features_from_surface` and `features_from_files` take 8 arguments
  (`clippy::too_many_arguments` allowed twice); `repo_root`, `brief`, `evidence`, `llm` and `tracker` go
  through almost every function; `UseCaseContext` is rebuilt to change one field for the rules.
* Largest files: `retrodoc-mcp/src/tools.rs` 1,059 lines, `retrodoc-llm/src/openrouter.rs` 990,
  `retrodoc-render/src/markdown.rs` 923, `retrodoc-pipeline/src/report.rs` 744, `chunks.rs` 706.
* `chunk_check.rs` (486 lines) verifies the boundary regexes the LLM proposes; since ADR 0027 the five
  parsed languages no longer use it.

# Approach
One deliverable per point, each a `refactor(...)` commit:
1. Read the per-unit passes side by side and write down what really is the same (it may be less than it
   looks); only then choose between a generic runner (trait or function taking the unit, the fingerprint,
   the call and the grounding) and smaller shared helpers.
2. A run context for `generate` (repository root, configuration, providers, tracker, brief, evidence)
   instead of long argument lists.
3. The reading passes: one module for the joint reading and its two single-purpose variants, if the
   standalone commands `glossary` and `entry-points` are kept; else remove them (a decision for the user).
4. The regex boundaries: measure which extensions of the smoke and benchmark repositories still reach
   `chunk_check`; keep, or fall back on blank lines for unparsed languages and remove the calls.
5. Split the largest files along what they already hold (tests apart, one tool per file).

# Resources
* `crates/retrodoc-pipeline/src/batched_read.rs`, `features/`, `use_cases/`, `rules/`, `confidence/`
* `crates/retrodoc-cli/src/commands/generate/mod.rs`, `commands/usage.rs`, `commands/workspace.rs`
* `issues/done/refactor_code_hygiene.md` (what was already done, and how)
* ADRs 0003, 0005, 0013, 0027 and 0028

# Hints
* After `single_pipeline_path`: do not refactor code about to be removed.
* An abstraction that hides the differences between passes behind many parameters is worse than the
  duplication: stop where the passes really differ.
* The fingerprints must not change, or every user's next run redoes everything.

# Tracking

Plan: validated

## Understanding
Share what the passes really have in common, make `generate` thread one run context, share the leftovers of the
three reading passes; no behaviour change, fingerprints byte-identical. Out of scope: features and confidence loops
(sequential with the previous slices in the prompt; no per-unit print), the split of the largest files (over the
size limit: ~2,300 moved lines counted twice; new issue). `chunk_check` measured: no extension of
`delivery_router`, `linkding` or `lobsters` reaches it (all parsed by tree-sitter, or not walker sources).

What is the same (read side by side): use cases and rules run the same loop (`Item { key, print, plan }`,
`Plan::{Reuse, Unreadable, Ask}`, `buffered(concurrency)`, skip/begin, partial save on each answer and on error,
artifact saved before the prints); they differ in the payload (rules carry the set-aside entries) and in what an
unanswered or unreadable feature keeps (use cases drop the previous ones, rules keep them). Features, use cases,
rules, actors and domains share the fingerprint tail (brief when not empty, then `model_part`); use cases and rules
share the file part (`path\nhash`, or `unreadable`) and the key `"{domain}/{slug}"`.

## Plan
1. [x] Pin and share the fingerprint parts: tests pinning the print of one unit of features, use cases and rules
   (literal hash); one helper for the brief + model tail and one for the cited file contents, plus the feature key;
   verified by the pinned prints unchanged (~200 lines).
2. [x] One per-feature driver for use cases and rules: the `Item`/`Plan` loop, its progress, partial and final
   saves in one function; each pass gives its planning, its call + grounding and its policy for an unanswered
   feature (keep or drop the previous ones); verified by the existing use cases and rules tests (resume, partial
   save, concurrency) unchanged (~350 lines).
3. [x] A run context for `generate`: `Run { repo_root, config, walked, llm, tracker, force }` plus `Signals`
   replace `Passes` and the long argument lists of `generate/mod.rs` and `contexts.rs` (the `too_many_arguments`
   allow of `features_from_surface` goes); the concurrency becomes an argument of `build_use_cases` /
   `build_rules` instead of a field of `UseCaseContext`, so the context is no longer rebuilt for the rules;
   verified by `just check` and a `generate` on a copy of a saved `delivery_router` run making no LLM call
   (~300 lines).
4. [ ] The reading passes share their leftovers in `batched_read.rs`, the modules staying where they are: the
   loop building the pending chunks (read, hash, compare, chunk), the attribution of an answer to a cited file,
   the seeding of `Retention::Others`; verified by the glossary, entry points and business read tests unchanged
   (~250 lines).
5. [ ] `chunk_check` kept: record the measure (extensions that reach it on the three repositories) in ADR 0027
   and `docs/crates/pipeline.md`; verified by `just check` (~20 lines).

## Decisions
- Mode: autonomous
- Base commit: aee4cc9598a94f40ff9326001cee6fdd5aaf0341
- Standalone `glossary` and `entry-points` (user, 2026-10-29): kept; their shared pieces go to `batched_read.rs`.
- `chunk_check` (user, 2026-10-29): kept; the measure goes in ADR 0027.
- Split of the largest files (user, 2026-10-29): a new issue, queued right after this one.
- Joint reading prompt lacking the routing-file rule of `entry-points` (user, 2026-10-29): a follow-up issue.

## Deliverable 1
- done: three tests pin the saved print of one unit of features, use cases and rules (literal hashes);
  `ProductBrief::print_part` replaces the six copies of the brief part (features, placement, use cases, rules,
  actors, domains), `fingerprints::file_part` the two copies of the cited file part (use cases, rules),
  `fingerprints::feature_key` the three of the key (use cases twice, rules).
- checks: see the table below.
- red: none (no requirement: pure refactor); the pins first failed on their placeholder, showing the real prints
  (`orders/-`, `billing/idle`, `delivery/orders`), then held through the refactor.
- not verified: the actors and domains prints are not pinned (their change is the same one-line expression the
  pinned passes use).
- Code review: no findings.
- decisions needed: none.

## Deliverable 2
- done: `per_feature.rs` holds the loop of the use cases and rules passes (`Plan`, `Unit`, `FeaturePass`,
  `open`, `run`): planning before any call, `buffered(concurrency)`, reuse and progress, the checkpoint after
  each answer and on an error; each pass keeps its call, its grounding, what it keeps of a feature, its partial
  write and its final save. The two policies the passes differ on (a feature unreadable or not answered keeps
  its saved results, and the run is saved then) are one constant, `KEEP_UNANSWERED` (rules yes, use cases no).
- red: none (no requirement: pure refactor); the existing tests (resume after a kill or a failure, partial
  saves, concurrency, reuse) and the pinned prints of deliverable 1 held unchanged.
- not verified: two differences no test sees: the log of an unreadable feature of the use cases says
  `use cases skipped: ...` (was `feature skipped: ...`), and a partial save of the rules lists the saved rules
  of the features not reached in the order of the features (was the order of the saved file); the final save
  is unchanged.
- Code review: no findings.
- decisions needed: none.

## Deliverable 3
- done: `Run { repo_root, config, walked, llm, tracker, force }` (with `Run::concurrency(pass)`) replaces
  `Passes` and the argument lists of `generate/mod.rs` and `contexts.rs`; `Signals` is built once and passed
  whole; the `too_many_arguments` allow of the CLI goes (the one of `build_features_from_surface`, in the
  pipeline, stays: its eight inputs are those of the pass). `concurrency` leaves `UseCaseContext` for an
  argument of `build_use_cases` and `build_rules` (the replays and the tests pass `1`, as the default context
  gave), so the context is no longer rebuilt for the rules and is borrowed by `finish_chain`.
- red: none (no requirement: pure refactor); the existing tests held, and a `generate` on a copy of the saved
  `rsa-dr` run (`~/retrodoc-runs/pass-runner-noop`, `base_url` pointing at a closed port) made no LLM call
  and wrote no file.
- not verified: the `contexts` path of `generate` through the binary (no saved `contexts` run of
  `delivery_router`); its pipeline side is covered by `two_runs_tests.rs`.
- Code review: no findings.
- decisions needed: none.
