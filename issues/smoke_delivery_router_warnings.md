# Fix the two pipeline warnings that fail the delivery_router smoke test

# Goal

Make the smoke test on `delivery_router` (a small Ruby repo, 9 source files) conclusive by fixing the causes
of the two `WARN` lines of its first run, not by silencing them. Criterion 2 of the smoke test ("no `WARN` in
either run") is the only one that fails. The two warnings are linked, and the first one hides the core of the
application from the generated docs.

# Findings (investigation of 2026-10-06, local Ollama, `qwen3.6:35b-a3b`, `reasoning_effort = "none"`)

## 1. "clustering left 6 file(s) unassigned" (domains pass)

* What happened: the saved clustering assigns three sub-domains to **single files** (`order.rb`, `customer.rb`,
  `delivery_router.rb`) and no folder. The six other files of `lib/delivery_router/` (`rider.rb`, `riders.rb`,
  `router.rb`, `restaurant.rb`, `orders.rb`, `locatable.rb`) fall into the "uncategorized" bucket, so they get no
  feature and no use case: the report lists them under "Undocumented code", and a headless agent using the MCP
  server could not read `rider.rb`, which holds the delivery time calculation, because no doc cites it. The
  repair worked as designed (100% coverage by construction, ADR 0004); the loss is in the docs, not in the run.
* Why: the model is shown the **modules** (three folders, `""`, `lib` and `lib/delivery_router`) and is asked
  to copy their paths. The surface section of the same prompt lists the entities **with their file paths**
  (`Order (lib/delivery_router/order.rb)`, `Customer (…/customer.rb)`). The failing answers cite only files
  and no module at all.
* Measure: the exact prompt rebuilt from the run's artifacts and replayed 20 times on the same model.
  With the real prompt, 3 answers out of 20 left files unassigned (3, 5 and 6 files), all three citing no
  module; 3 answers out of 20 also cited invented file paths (dropped by the pipeline) and 1 was unparseable.
  With the file paths removed from the surface section: 0 out of 20 on all counts. The sample is small, one
  repo and one model, so this is suggestive (Fisher exact test, p ≈ 0.1), not proven.
  Without any surface section, 2 answers out of 6 were incomplete as well (an earlier, smaller batch), so
  removing the paths probably isn't enough on its own.

## 2. "LLM answered with no use case" (use cases pass, `attempt=1`)

* What happened: the feature `rider-assignment-and-routing` is grounded on a single file, `lib/delivery_router.rb`:
  a facade of seven lines (four `require` and a `self.new` that returns `Router.new`). The first attempt
  answered with no use case, the second returned one (two steps, confidence 100%) whose feature description,
  "calculates optimized delivery paths", is not in that file: the routing code is in `router.rb`, one of the
  six uncategorized files.
* So this warning is most likely a consequence of finding 1: a feature built on a facade whose real code was
  left out has nothing to derive. It is also a second problem in itself: the retry produced a use case the
  code does not back, and the confidence pass scored it 100%.
* The warning is emitted on `attempt=1` even though the pass recovered on `attempt=2`, and the smoke test
  counts it. Smoke logs of other repos mostly show the other case, a feature answered empty on both attempts
  (a legitimate "no use case").

# Approach

Fix finding 1 first, rerun the smoke test, then see whether finding 2 is still there. Candidates for 1, which
can be combined:

1. Prompt: stop showing file paths in the surface section of the clustering prompt (show the entity and the
   module that contains it). Check with the same replay: expect 0 unassigned out of 20.
2. Repair before bucketing: for the files left unassigned, either ask the LLM once where to place them (a small
   prompt: the files with their summaries and the domains found), or inherit mechanically the domain of the
   assigned files of the same folder when they all agree. "Uncategorized" would keep only what remains.
3. Retry the clustering once when it is incomplete (the answer varies from one call to the next).

For 2: decide whether a first empty answer that the retry recovers deserves a `WARN`, or only the case where
both attempts fail, and look at why the retry returned an ungrounded use case with full confidence.

# Resources

* `crates/retrodoc-pipeline/src/domains/` (`prompt.rs`, `coverage.rs`: `expand_to_files`, `enforce_coverage`)
* `crates/retrodoc-pipeline/src/surface.rs` (`Surface::prompt_section`, entities with their files)
* `crates/retrodoc-pipeline/src/use_cases/mod.rs` (`ask_use_cases`, the two attempts)
* ADR `0004` (domains by directory, coverage by construction), `0008` (surface extraction)
* `.claude/skills/smoke-test/` (criterion 2) and the run logs under `/tmp/retrodoc-smoke-delivery_router.*`

# Hints

* Method of the replay: rebuild the clustering prompt from the artifacts of the smoke clone (repo map, surface,
  existing doc titles), post it to the local server N times, and count the files no assigned path covers (a
  path covers a file if it is that file, a parent folder, or the root `""`; a trailing `/` is ignored, as
  `Path::starts_with` does). The scripts live in `/tmp` and are not committed; a unit test with a fake
  provider is the right regression test for the repair, as for the other passes.
* Don't hand-edit the generated artifacts to make the run pass (smoke test rule), and don't just lower the
  log level of the domains warning: the files really are undocumented.
* The model answers differently each time, so judge a fix on several runs, not on one.
* After the fix, run `just smoke ~/Code/delivery_router` and check that the report no longer lists
  `rider.rb` and `router.rb` as undocumented.

# Tracking

1. [x] Surface section of the clustering prompt without file paths (entity + containing module, same for resources)
2. [x] LLM repair of the files left unassigned by the clustering, before the "uncategorized" bucket (validated, cached, ADR 0004 updated)
3. [x] Use cases pass: `WARN` only when both attempts fail
3b. [x] Actors pass: "no actors identified" is an `info`, not a `WARN` (a small library has none; it failed the smoke test on every run)
4. [ ] Validation: `just smoke ~/Code/delivery_router` + 20 replays of the clustering prompt
5. [ ] Conditional: ungrounded use case scored 100% (only if still present after 4)

## Decisions

* Repair by one small LLM call (unassigned files with their summaries + the domains found), not by mechanical inheritance.
* The "retry the clustering once" candidate is dropped in favor of the repair call; reconsidered only if the smoke test still shows incomplete answers.
* The smoke test may run on the local Ollama.
* The actors "nothing found" line goes to `info` (decided after two smoke runs where it was the only constant warning).
* Deliverable 5 is handled only if the smoke test shows the problem persists.
