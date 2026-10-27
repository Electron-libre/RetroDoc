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
