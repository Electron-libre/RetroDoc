# Settle the experimental settings: default, keep or remove

# Goal
Several settings were added as experiments and left as options: each one doubles the configurations nobody
measures together, and stays in the code, the docs and the tests. For each, decide with the figures:
becomes the default, stays an option with a stated use, or is removed.

# Findings (project review of 2026-10-12)
* `[pipeline] body_lines` (off, ADR 0027): on linkding, three local runs per setting, N=5 cuts the input
  tokens of the entry points pass by 42% and of the glossary by 40%; the entry points found stay within the
  spread; the glossary lists fewer attributes in all three runs (77-94 against 103-132)
  (`issues/done/tree_sitter_structure.md`). Since then `business-read` reads those files for both passes
  (ADR 0028): the gain was not measured again with it.
* `[brief] evidence` (off, ADR 0019): fewer features and no gain in the benchmarks; it has its own issue,
  `issues/done/brief_injection_experiments.md`.
* `[pipeline] business_read` (on, ADR 0028): the switch exists to turn it off; no known case where that is
  wanted.
* `[pipeline] rules` (off, ADR 0021): about 42% of the input tokens of a run, rules still noisy
  (`issues/done/rules_deduplication_and_noise.md`).
* `[pipeline] domains_from`: `issues/done/surface_domains_default_decision.md` and `issues/done/single_pipeline_path.md`.
* `--confidence-sample N`, `--no-confidence`, `--max-files`, `llm.batch_chars`, `llm.structured_output`:
  options with a use, listed here so the review is complete.

# Approach
1. `body_lines`: measure again with `business-read` on (`entry_points_replay`, entity and attribute counts,
   three runs; check the ledger of measures (`CLAUDE.md`) for saved runs with `business-read` on first), and check on the use cases downstream whether the lost attributes matter (the vocabulary
   given to the use cases is entity names). Then: default N, or option, or removed.
2. `business_read`: remove the switch if no case needs it (the passes already read alone when their models
   differ).
3. `rules`: decide the default once `rules_deduplication_and_noise` and
   `one_code_reading_for_use_cases_and_rules` are done.
4. Write the outcome of each in its ADR's follow-up (a new ADR when one is superseded), the default
   `retrodoc.toml` of `init`, the README and `docs/crates/pipeline.md`.

# Resources
* `crates/retrodoc-core/src/config.rs` (`PipelineConfig`, `BriefConfig`, `write_default`)
* `issues/done/tree_sitter_structure.md`, `issues/done/single_read_per_business_file.md`
* ADRs 0019, 0021, 0027 and 0028
* `crates/retrodoc-pipeline/examples/entry_points_replay.rs`

# Hints
* `body_lines` is part of the file hash of the reading passes: a new default redoes them once for everyone.
* Removing a key: `retrodoc.toml` files that still name it must get a clear message.
* Easier to measure after `benchmark_passes_on_fixed_inputs` and `sampling_temperature_for_steadier_runs`.

# Tracking

Plan: validated (done 2026-10-27)

## Understanding
Decide, figures in hand, the fate of `business_read`, `rules` and `body_lines`: default, option with a stated use,
or removed, and write it in the code, the `init` default, the README, `docs/crates/pipeline.md` and the ADRs.
Out of scope: `[brief] evidence` (`brief_injection_experiments`), `domains_from` (its two issues), the options with a use.
Spec: `specs/pipeline-settings.md` (SET).

## Plan
1. [x] `business_read`: kept as an option with its stated use (a small library, where the joint reading finds
   fewer entry points); verified by a config test; makes true SET-3; ADR 0028 follow-up, README, `pipeline.md`.
2. [x] `rules`: on by default; the smoke test follows the default (`--rules` becomes `--no-rules`), the
   benchmark keeps it off (it does not judge the rules); verified by config and `init` tests and the harness
   tests; makes true SET-1, SET-2; ADR 0021 follow-up, README, `pipeline.md`, skills.
3. [x] Measure `body_lines` with `business-read` on: frozen linkding inputs (roles, business files, brief),
   `business-read` + glossary + entry points, 3 runs off and 3 at N=5; `entry_points_replay`, entity and
   attribute counts, entity names compared between settings (the vocabulary of the use cases); figures here and
   in the ledger. Stop on the figures for the decision.
4. [x] `body_lines`: default N, option, or removed (with the message of a removed key), per the decision of 3;
   verified by config tests; ADR 0027 follow-up.

5. [x] Fixes of the `high` review of the whole issue: config errors keep their line, the benchmark turns the
   rules off whatever the `[pipeline]` section holds, the smoke test refuses `--rules` and a `--no-rules` it can't apply.

## Decisions
- Answers of 2026-10-27: 1 a (`business_read` kept, stated use), 2 a (`rules` on by default), 3 a (measure on
  DeepSeek `deepseek-chat`).
- Answer on the measure (2026-10-27): `body_lines` removed (the gain is about 4% of a run, the entry points
  only move down).
- Mode: autonomous (stops anyway before the paid run of 3 and on its figures)
- Base commit: 96e98285237dd7a023729e0ee27c05d32d0d499c

## Deliverable 1 (`business_read` kept)
- done: SET-3 traced by its own config test; the stated use written in the doc comment, the README sample,
  `pipeline.md` and ADR 0028 (follow-up).
- red: none (the behaviour was already true; the test moved out of the `domains_from` test).

## Deliverable 2 (`rules` on by default)
- done: `[pipeline] rules` defaults to true (SET-1, SET-2 traced); the smoke test takes `--no-rules` instead of
  `--rules`; the benchmark writes `rules = false` (tested by `test_benchmark.rs`); README sample, `pipeline.md`,
  `ARCHITECTURE.md`, `PLAN.md`, both skills and ADR 0021 (follow-up).
- red: `assertion failed: Config::default().pipeline.rules` (config tests), `FAIL - turns the rules pass off` (harness).
- Consequence: a project whose `retrodoc.toml` has no `rules` key runs the rules pass at its next `generate`;
  one written by `init` before keeps `rules = false`.

## Measure of deliverable 3 (`body_lines` with `business-read` on, 2026-10-27)
linkding at 27b7303, roles, business files and brief of `frozen/linkding`, `deepseek-chat`, `business-read` then
`glossary` (no call left: the joint reading covers its files) then `entry-points`, 3 runs per setting
(`~/retrodoc-runs/settle_experimental_settings-71fdad9/`, about $0.10 in all; counts by
`scripts/body_lines_counts.py`, entry points by `entry_points_replay`).

| | off | N=5 |
|---|---|---|
| tokens in (business-read + entry-points) | 70.9k (52.5k + 18.4k), 22 calls | 43.9k (33.2k + 10.7k), 14 calls: -38% |
| entry points | 117 (112-121) | 120 (108-137) |
| groups of the reference found | 93% (93-93) | 90% (86-93) |
| entries in reference files | 89% (88-92) | 84% (73-91) |
| entities / attributes / associations | 20 (19-21) / 140 (134-147) / 22 (20-23) | 16 (15-17) / 101 (100-103) / 17 (16-18) |
| attributes of the 8 model entities | 65 (65-66) | 68 (65-70) |

* The attributes lost are those of the entities N=5 no longer lists: the Django forms (`BookmarkBundleForm`,
  `BookmarkSearchForm`, `GlobalSettingsForm`, `UserProfileForm`), sometimes `WebsiteMetadata`. The 8 model
  entities are in every run with as many attributes or more. None of the names lost appears in the frozen
  features or use cases (`BookmarkSearch` does, twice, and an `off` run alone lists it).
* Downstream, the reading passes are a small share of a run: 27k tokens saved is about 4% of the input of a
  linkding `generate` with the rules on (use cases and rules about 280k each).
* Entry points: one N=5 run finds one group fewer and 73% of its entries in reference files; the other two are
  as `off`. Within the spread of three runs, but the only move is downwards.

## Deliverable 4 (`body_lines` removed)
- done: the key, `RoleMap.body_lines`, the body view of `Splitter` (`with_body_lines`, `keyed`, `elide_bodies`) and
  their tests are gone; a `retrodoc.toml` naming the key fails to load with `[pipeline] body_lines was removed: …;
  delete the line from retrodoc.toml` (SET-4, `REMOVED_KEYS` in `config.rs`); spec `accepted`; ADR 0027 follow-up,
  `pipeline.md`, `PLAN.md`. The file hash of the reading passes is unchanged for the default (whole files): no redo.
- red: `called Result::unwrap_err() on an Ok value` (the key was still read).

## Deliverable 5 (fixes of the `high` review, `96e9828..a9ad181`)
- done: `Config::load` deserializes as before and looks for a removed key only when that fails, so a mistyped key
  keeps `line 3, column 1` in its error (test under SET-4); `without_rules` of `benchmark.rs` sets
  `rules = false` in `[pipeline]` whatever it holds (key absent, other spacing, no section: harness tests);
  `smoke.rs` dies on `--rules` (gone) and on a `--no-rules` with no `rules = true` to turn off; the spec's
  non-goal says the benchmark always turns the rules off.
- red: `invalid TOML config: unknown field rulez…` with no position; `FAIL - turns the rules pass off when the
  key is absent`, `… without a [pipeline] section`.
- Code review `low` of the fixes: the end of `[pipeline]` is a `[name]` or `[[name]]` header only, not an array
  value on its own line (fixed; quoted headers are not handled, `retrodoc.toml` has none); `REMOVED_KEYS` needs a section that denies unknown fields (documented on the constant).
  Not fixed: `--no-rules` of the smoke test stops when `init` wrote no `rules = true` (loud on purpose; `init`
  writes it, SET-2), and the two scripts keep their own patch (separate `rust-script` files).
- Not fixed, by design: `REMOVED_KEYS` looks one section deep; no nested key is removed today, a deeper one
  extends it when it comes.
