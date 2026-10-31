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
