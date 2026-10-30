# Give the rules replay the evidence `generate` gives the rules pass

# Goal

`pass_replay rules` measures the rules pass with no extracts of the signals, while `generate` gives that pass the
extracts closest to each feature whenever `[pipeline] rules` is on. The rules backed by a test or a doc section may
therefore be missing from the replays and not from a real run. Make the replay give what `generate` gives, so the
rules baselines measure the pass as it runs.

# Findings (code reading of 2026-10-28, `brief_injection_experiments` deliverable 1)

* `benchmark/replay_use_cases.rs` `prepare_with_tree` fills `UseCaseContext` with `..UseCaseContext::default()`:
  `rule_evidence` is empty in every rules replay (`benchmark/replay_rules.rs`).
* `generate` gives `rule_evidence` the extracts of the signals when the rules pass is on (`EvidenceFor` in
  `crates/retrodoc-cli/src/commands/brief.rs`); `rules/input.rs` puts them in the prompt.
* The rules baseline of the ledger (2026-10-26: kept 270 (256-280), reference rules stated 11.4 of 29) was
  measured that way.
* `benchmark::tree_evidence` (added by `brief_injection_experiments`) builds those extracts from the
  `source_run` tree, its generated docs left out.

# Approach

1. Set `rule_evidence` from `tree_evidence` in the rules replay (a test proving the extracts reach the prompt).
2. Rerun the rules baseline on frozen linkding (3 runs, about $0.3) when a rules issue next needs it, and compare
   with the 2026-10-26 figures.

# Milestone
Business rules quality

# Resources

* `crates/retrodoc-pipeline/src/benchmark/replay_rules.rs`, `replay_use_cases.rs`, `frame.rs`
* `crates/retrodoc-pipeline/src/rules/input.rs`
* `skill:quality-benchmark`, the ledger of measures (`CLAUDE.md`)

# Hints

* Share one rules baseline with `rules_plumbing_and_screen_noise` rather than paying two.
