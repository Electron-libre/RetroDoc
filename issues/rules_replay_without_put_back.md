---
rank: 950
why: 'low: no effect on the current frozen inputs; after `rules_plumbing_and_screen_noise`, whose measures it would only matter to on a run that puts rules back'
---
# Give the rules replay the `[rules]` put-backs `generate` gives the rules pass

# Goal

`pass_replay rules` runs the rules pass with an empty `UseCaseContext::put_back`, while `generate` sets it from
`[rules]` of `retrodoc.toml` (`keep_features`, `keep_rules`). On frozen inputs whose source run puts features or
rules back, the replay would set aside what a real run publishes, and count fewer rules than the pass gives. Make
the replay read the put-backs of the source tree, so it runs the pass as `generate` does.

# Findings (code reading of 2026-10-31, `high` review of `rules_replay_without_evidence`)

* `benchmark/replay_use_cases.rs` `prepare_with_tree` fills `UseCaseContext` with `..UseCaseContext::default()`:
  `put_back` is empty in every rules replay.
* `generate` sets `put_back: run.config.rules.clone()` (`crates/retrodoc-cli/src/commands/generate/mod.rs`).
* The source run of the frozen linkding inputs has no `[rules]` section: the current rules baselines are not
  affected.

# Approach

1. Read `[rules]` of the `source_run` tree (as `tree_evidence` reads its `[ingest]` and `[output]`, tolerant of
   settings removed since) and set `put_back` in the rules replay; a test proving a feature of `keep_features` is
   not set aside in the replay.

# Milestone
Business rules quality

# Resources

* `crates/retrodoc-pipeline/src/benchmark/replay_rules.rs`, `replay_use_cases.rs`, `frame.rs`
* `specs/rules-set-aside.md`
* `crates/retrodoc-core/src/config.rs` (`RulesConfig`, `Config::load_ingest_and_output`)

# Tracking

Plan: validated

## Understanding
`pass_replay rules` must run the rules pass with the `[rules]` put-backs of the `source_run` tree, as `generate`
does. Out of scope: the other replays (no other pass reads `put_back`). No spec: benchmark tooling, no spec covers
the replays.

## Plan
1. [x] Put-backs in the rules replay: `Config::load_ingest_and_output` becomes `Config::load_saved_run`, keeping
   `[rules]` too (still tolerant of removed settings); `frame.rs` gets `tree_put_back(tree)` (default without a
   `retrodoc.toml`), `replay_rules` sets `context.put_back` from it; docs of `docs/crates/pipeline.md`; verified by
   a `replay_rules` test where a feature of `keep_features` whose LLM answers "about tests" still gets its rules,
   and a `config.rs` test that the saved run keeps `[rules]`; ~100 lines.

## Decisions
- Mode: autonomous
- Base commit: ad9e91ef9fa90e7510a721dec33d0c60e73747ba
- The `config.rs` test of the plan was dropped: the replay test reads `[rules]` through a `retrodoc.toml` that names
  a removed setting, so it proves `Config::load_rules` reads `[rules]` and stays tolerant.

## Plan (after the `high` review)
2. [x] Review fixes: `[rules]` read on its own (`Config::load_rules`), so the features and use cases replays
   and `--evidence off` don't parse it; the replay test also covers `keep_rules`; doc comment rewrapped.

## Summary of deliverable 1
- done: `replay_rules` sets `context.put_back` from `[rules]` of the source tree (`tree_put_back`,
  `Config::load_saved_run`, renamed from `load_ingest_and_output`); docs of `pipeline.md` and `pass_replay`.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clean |
| specs | no spec (benchmark tooling) |
| docs | `docs/crates/pipeline.md`, `pass_replay` doc |
| code review | low: no findings |

- red: none (no spec); test `a_rules_replay_states_the_rules_of_a_feature_the_source_tree_puts_back` failed with
  `published` 0 instead of 1 before the code.
- not verified: no replay on the frozen linkding inputs (no `[rules]` in their source run: no effect expected).
- Code review: no findings
- decisions needed: none.

## Summary of deliverable 2 (`high` review of `ad9e91e..4ac0ef2`, 8 findings)
- done: `load_saved_run` reverted to `load_ingest_and_output` plus `Config::load_rules` (one `load_sections`
  helper); `tree_put_back` reads `[rules]` only; test covers `keep_features` and `keep_rules`.

| check | result |
|---|---|
| tests | `just check` green |
| lint | clean |
| specs | no spec |
| docs | `docs/crates/pipeline.md` |
| code review | low: no findings |

- red: none (no spec); with `keep_rules = ["order-9"]` the test fails with `published` 0 instead of 1.
- `high` findings: fixed 3 (`[rules]` parsed by every replay), 4 (`--evidence off` reads the config), 7
  (`keep_rules` untested), 8 (long doc line). Not fixed: 1 (put-backs read from the live source tree, as
  `tree_evidence` reads it: the source runs are kept as they are), 2 (a `keep_rules` slug may name another rule
  once the feature is stated again: a non-goal of `specs/rules-set-aside.md`), 5 (title says nothing of the
  put-backs: no frozen source run has `[rules]`), 6 (config parsed once per run: negligible next to the LLM calls).
- not verified: no replay on the frozen linkding inputs.
- Code review: no findings
- decisions needed: none.
