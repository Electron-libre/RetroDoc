# Keep a ledger of measures and plan the queue's measures together

# Goal
Measures cost budget (hosted DeepSeek runs) and hours (local runs), and the agent redoes some of them each
session because nothing tells it what was already measured, at which commit, with which model, and where the
artifacts are. Expected outcome: a ledger of measures and reusable artifacts that the agent reads before any
paid or long run and updates after it, and one plan of the measures the open issues need, grouped so that a
run serves every issue it can.

# Findings (2026-10-27)
* The run directory outside the repo (the one `docs/dev/commands.md` uses for saved runs and frozen inputs)
  holds 130 entries (2.7 GB) with ad hoc names (`exp-ld-2-r3`, `rd-agree2`, four `tmp.*`). No file says which
  commit, model, setting or issue a run belongs to, nor whether it is still needed. Only the frozen inputs have
  a manifest.
* Results live in the `Findings` and `Measure` sections of the issues, open and done: knowing whether a measure
  exists means reading all of them.
* 10 of the 14 open issues of `issues/queue.md` plan measures, each with its own runs (often "3 runs per
  setting"), although several need the same reference point:
  * a baseline `pass_replay` of each pass (3 runs, DeepSeek, frozen inputs) is the "before" of
    `measure_model_split_per_pass`, `settle_experimental_settings` and `brief_injection_experiments`; it stays
    valid while the pass's prompt and code and the frozen inputs do not change;
  * `measure_model_split_per_pass` plans full benchmarks (about $0.30 a linkding run, 2 series, 3 runs per split)
    where a replay of each pass with the other model costs a few cents;
  * `rules_plumbing_and_screen_noise`, `parsed_references_for_ranking_and_slices` and the first steps of
    `contexts_residual_size_and_names` need no new LLM run, only saved outputs;
  * `contexts_residual_size_and_names` (step 3) and `merge_form_screens_into_their_use_case` both need a new
    complete run of the same large Rails application: one run can serve both;
  * `smoke_warnings_business_paths_sources_and_empty_use_cases` counts warnings over several local smoke runs
    of `delivery_router`, which other issues run anyway.
* 77 commits since 2026-10-20, many of them measures or records of measures (`docs: record ...`,
  `test(pipeline): measure ...`).

# Approach
1. Ledger: a Markdown file kept with the runs, outside the repo (user's choice, 2026-10-27), so it may name
   private repositories and paths. One line per measure: date, commit, issue, repository, model, command
   (pass replay, benchmark, smoke, no-LLM example), runs, cost, key figures, artifact path. A section of
   reusable artifacts (frozen inputs, complete runs a no-LLM measure can read, runs a manifest points to and
   that must be kept), and a section of planned measures.
2. Backfill it from the open and done issues and from the run directory; list the orphan runs (`tmp.*`, runs
   no issue cites) for the user to decide, delete nothing.
3. Rules: `CLAUDE.md` and the skills `quality-benchmark` and `smoke-test` (project-owned, not synced) say to
   read the ledger before a paid or long run (reuse a measure of the same pass at the same inputs), to append a
   line after, and to keep asking before a paid run. The synced skills (`issue-workflow`,
   `prioritize-issues`, `create-issue`) cannot be edited here: candidates are a project rule in `CLAUDE.md`
   ("update the planned measures when an issue is created or the queue is re-ranked") or a change proposed to
   the central harness.
4. Plan: write the shared plan of the queue's measures in the ledger (the groups of the Findings), and amend
   the Approach of the issues it changes (e.g. replays instead of full benchmarks in
   `measure_model_split_per_pass`), for the user to validate.
5. Optional: name new runs after their issue and commit (a convention in the skills), or let `just benchmark`
   and `pass_replay` append the ledger line themselves.

# Resources
* `docs/dev/commands.md` (saved runs, `freeze_inputs`, `pass_replay`), skills `quality-benchmark` and
  `smoke-test`
* `docs/dev/agent-harness.md` (which skills are synced copies)
* `issues/queue.md` and the open issues named above; `issues/done/benchmark_passes_on_fixed_inputs.md`,
  `issues/done/benchmark_spread_and_judge_bias.md`

# Hints
* The ledger is outside git: it may name confidential repositories, but committed files (skills,
  `CLAUDE.md`, issues) must still name them only generically.
* The use cases and rules replays read the code tree of `manifest.source_run`: the ledger must mark those
  runs as kept.
* A measure is reusable only at the same pass code, prompt, model and inputs: record the commit and the
  frozen inputs it used, not only the date.

# Tracking

Plan: validated

## Understanding
A ledger outside the repo (`~/retrodoc-runs/LEDGER.md`) that says what was measured (commit, model, inputs,
cost, figures, artifacts), backfilled from the issues and the run directory; rules that make the agent read it
before a paid or long run and append to it after; one plan of the queue's measures. No run is deleted, no new
LLM run is made by this issue.

## Plan
1. [x] Ledger skeleton and backfill: `~/retrodoc-runs/LEDGER.md` with the measures of the open and done issues
   (date, commit, issue, repo, model, command, runs, cost, key figures, artifact), the reusable artifacts (frozen
   inputs, their `source_run` marked kept, complete runs a no-LLM measure reads) and the orphan runs for the user;
   verified by a cross-check: every run dir is either cited by a line or listed as orphan. Outside git, no commit.
2. [x] Rules: `CLAUDE.md` and the project skills `quality-benchmark` and `smoke-test` say to read the ledger before
   a paid or long run, append a line after, keep asking before a paid run, and name new runs `<issue>-<short commit>-<n>`;
   `CLAUDE.md` also says to update the planned measures when an issue is created or the queue re-ranked;
   verified by `just test-harness` and `just check`.
3. [x] Shared plan: the "Planned measures" section of the ledger (groups of the Findings) and the amended Approach of
   the issues it changes (`measure_model_split_per_pass` replays instead of full benchmarks, the no-LLM issues pointed
   at saved runs, one complete large Rails run shared by two issues); verified by `just check-issue` on each amended issue.
4. [x] Fixes of the `high` review of `c9b98ed..10ad7e1`.

## Decisions
- Mode: autonomous
- Base commit: c9b98edd9fe107204e2237ebdb61c26f3f89accf
- Synced skills: a project rule in `CLAUDE.md`, no change proposed to the central harness (user, 2026-10-27).
- Runs: a naming convention `<issue>-<short commit>-<n>` in the skills only; no tool appends to the ledger (user, 2026-10-27).
- Ledger file: `~/retrodoc-runs/LEDGER.md` (user, 2026-10-27).
- Amended Approaches validated by the user (2026-10-27); issue closed.

## Summary of deliverable 1
- done: `~/retrodoc-runs/LEDGER.md` (outside git): 18 lines of measures (the detailed ones from 2026-10-13 on,
  earlier ones grouped as stale), 13 reusable artifacts, orphans. Cross-check: every top-level entry of the
  run directory is cited by a line or listed as orphan (companion `.log`/`.report.txt` files go with their run).
- Findings while backfilling:
  * 63 of the 130 entries are empty directories (no file): the runs of 2026-10-01 to 10-14 lost their data
    when they left `/tmp`; their logs survive in `logs/`. Listed as orphans, safe to delete.
  * The saved context map of the large Rails application (`contexts.yaml`, run of 2026-10-13) is among them:
    steps 1-2 of `contexts_residual_size_and_names` cannot run without a new `contexts` run.
  * The figures of the 3-run domains and features replays of 2026-10-23 were not saved (only ranges in
    `second_application_in_benchmark`): the baseline of those two passes has to be rerun (cents).
  * Still valid baselines: use cases (final shape, 3 runs, 1c6f592), rules (7 runs, 2026-10-26, check the
    diff of `rules/` since).
- Code review: none (no file of the repository changed but this issue).
- red: none (data, not code).
- not verified: the commit of the measures before 2026-10-23 is the commit that recorded them, not always the
  measured one.

## Summary of deliverable 2
- done: section "Ledger of measures" in the skills `quality-benchmark` (reuse rule by `git diff` of the pass,
  append after, naming) and `smoke-test` (copy the clone before the next smoke run of the same repository
  deletes it); a paragraph in `CLAUDE.md` (read, reuse, append, ask before a paid run, update the planned
  measures when an issue is created or the queue re-ranked); one line each in `docs/dev/commands.md` and
  `docs/dev/agent-harness.md`.
- checks: `just check` ok, `just test-harness` ok.
- red: `FAIL - the skill reads and appends the ledger of measures` (`test_benchmark.rs`; same check added to
  `test_smoke.rs`).
- not verified: that the agent follows the rule in a later session (only the text is tested).

## Summary of deliverable 3
- done: "Planned measures" in the ledger, six groups: G1 saved runs only (`merge_form_screens` step 3,
  `rules_plumbing` step 1, `parsed_references` step 1, `smoke_warnings` first count), G2 baselines of each pass
  on frozen linkding (use cases valid at 1c6f592, rules to check, domains and features to rerun for cents),
  G3 model split by replays, G4 one `contexts` run of the large Rails application, G5 second application,
  G6 parked local runs. Approach amended in 7 issues: `measure_model_split_per_pass` (replays, then one
  benchmark of the chosen split), `contexts_residual_size_and_names` (map lost: a `contexts` run first),
  `merge_form_screens_into_their_use_case` (a complete run is saved: step 3 needs no LLM call),
  `rules_plumbing_and_screen_noise`, `smoke_warnings_business_paths_sources_and_empty_use_cases`,
  `brief_injection_experiments`, `settle_experimental_settings` (pointers to the ledger groups).
- checks: `just check-issue` passes on 6 issues; on `merge_form_screens_into_their_use_case` it only
  reports its `# Tracking` section (issue already in progress); its private path was replaced.
- Code review: none (issues only).
- red: none (plan, not code).
- decisions needed: the user validates the amended Approaches (especially G3: replays replace the full
  benchmarks of `measure_model_split_per_pass`) and decides on the orphans of the ledger (63 empty
  directories, `reconcile-replay-old/`, 540 MB).

## Summary of deliverable 4 (fixes of the `high` review)
- done (fixed):
  * reuse rule (skill `quality-benchmark`, ledger header): clean tree, `<module>` named (`use_cases` for
    `use-cases`), and the diff also covers `pass_replay.rs`, `src/benchmark`, `retrodoc-llm`, `benchmark/<repo>`;
  * `measure_model_split_per_pass`: `pass_replay` judges with the replayed model, so a fixed judge first;
  * skill `smoke-test`: a saved run answers a measure, never the validation of a change; the next smoke run
    also overwrites the logs, so copy them with the clone;
  * `merge_form_screens_into_their_use_case`: step 3 and the "On hold" text no longer contradict the ledger note;
  * `smoke_warnings_business_paths_sources_and_empty_use_cases`: saved logs give the counts, not the raw answers;
  * the issues no longer cite ledger group numbers (renumbering would break them), only "the planned measures";
  * the skill tests also check the **Before** and **After** rules.
- not fixed: the copy rule stays text; protecting the clone in `smoke.rs` is
  `issues/smoke_run_keeps_previous_clone.md` (new, ranked before `git_log_follow_renames`). The ledger rule
  is summarized in 4 places (CLAUDE.md, two skills, a line in `commands.md` and `agent-harness.md`): the two
  skills hold the detail, the others point to them.
- checks: `just check` ok, `just test-harness` ok.
- red: `FAIL - the skill reads and appends the ledger of measures` (`test_smoke.rs`: the smoke skill wrote "**before**" in lower case), fixed.
