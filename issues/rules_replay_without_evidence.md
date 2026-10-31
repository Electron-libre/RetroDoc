---
rank: 800
why: 'small, right before `rules_plumbing_and_screen_noise`: its rules measures should replay the pass with the evidence `generate` gives'
---
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

# Tracking

Plan: validated

## Understanding
Give the rules replay the extracts `generate` gives the rules pass (`rule_evidence`, from `tree_evidence` of the
`source_run` tree), on by default with `--evidence off` to reproduce the old baselines. Out of scope: the paid
rerun of the rules baseline (approach 2), left to `rules_plumbing_and_screen_noise` which shares it. No spec: the
replay is a measuring tool that no spec covers.

## Plan
1. [x] Rules replay with evidence: `replay_rules` takes the rule evidence and puts it in `UseCaseContext::rule_evidence`;
   `pass_replay rules` builds it with `tree_evidence` (`--evidence on|off`, default on, said in the title); docs of
   `pass_replay` updated; verified by a test of `replay_rules` whose fake LLM sees an extract of the evidence in the
   rules prompt.

## Decisions
- Mode: autonomous
- Base commit: 4670b62015c6093ec5aa5aa86ccbad30e91bfdc1
- Q1 a: `rules` takes the evidence by default, `--evidence off` reproduces the old baselines.

## Deliverable 1 summary
- done: `replay_rules` takes the rule evidence into `UseCaseContext::rule_evidence`; `pass_replay rules` gives
  `tree_evidence` of the source tree by default (`--evidence off` for the old baselines, said in the title);
  `code_read_overlap` keeps no evidence (it compares code only); `docs/dev/commands.md`, `docs/crates/pipeline.md`.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass (no spec: measuring tool) |
| docs | updated |
| code review | `low`: no findings |

- red: none (no spec): `a_rules_replay_gives_the_pass_the_extracts_closest_to_each_feature` failed with the rules
  prompts holding no extract.
Code review: no findings
- not verified: a real `pass_replay rules` run (paid; the rules baseline is left to `rules_plumbing_and_screen_noise`).
- decisions needed: none.
