# Measure a model split per pass with the quality benchmark

# Goal

Know whether spending a strong model on the passes that frame the documentation (brief, domains, features)
and a cheap or local one on the extraction passes (repo map, glossary, entry points, actors, confidence)
keeps the quality of one strong model for every pass, at what cost. `[llm.passes.<name>]` exists
(`issues/done/model_per_pass.md`, ADR 0023) and the split suggested in the README is a guess until this is measured.

# Findings

* The mechanism is delivered and unit tested with fake providers; no real `generate` has run with two models.
* `.claude/skills/quality-benchmark/benchmark.rs` only patches `[llm]` (`--model`, `--base-url`,
  `--provider`): it cannot ask for a split.
* Its `patch_config` drops every line starting with `provider =`, `model =`, `base_url =` or
  `reasoning_effort =`, wherever it is in the file: a `[llm.passes.*]` section would lose those keys.
* The recap of a run already gives calls and tokens per pass and per model, but no price.

# Approach

1. Let the benchmark script take the per-pass sections (for example a `--passes <file>` with the TOML to
   append, or `--pass domains=<model>` repeated) and keep them out of the `patch_config` removal.
2. Replay each pass 3 times with the other model on the frozen inputs (`pass_replay`, skill
   `quality-benchmark`) against the baseline of the same pass, listed in the planned measures of the ledger
   (`CLAUDE.md`): cents for domains and features, about $0.3 a series for use cases and rules, instead of 2
   series x 3 full benchmark runs per split (about $1.8 a split). Item 1 is only needed for the final check.
   `pass_replay` judges with the replayed model itself: give it a judge model of its own (fixed, e.g.
   `deepseek-chat`) before comparing models, or a more lenient judge passes for a better model.
3. Pick the split from the replays (quality per pass, calls and tokens per pass and model), then run one whole
   benchmark of linkding (3 runs) with that split as the final check, compared with `benchmark-table`.
4. Write the result in the issue and in the README suggestion: keep, change or drop the recommended split.

# Resources

* `issues/done/model_per_pass.md`, ADR 0023
* `.claude/skills/quality-benchmark/SKILL.md`, `benchmark.rs`, `test_benchmark.rs`
* `benchmark/linkding/reference.yaml`

# Hints

* Changing the model of a pass redoes it, but the benchmark clones afresh each run, so no cache is shared
  between series.
* A hosted run costs money (about $0.30 per run on linkding with DeepSeek, per the skill): ask before
  launching with a strong hosted model.

# Tracking

Plan: abandoned (closed by the user, see Decisions)

## Understanding
Measure whether `deepseek-v4-pro` (strong) on the framing passes and `deepseek-flash` (today's default, cheap) on
the rest keeps the quality of the strong model, at what cost, by pass replays on linkding and lobsters then one
whole benchmark of the chosen split. Out of scope: local models (hours per series), changing any pass.
Finding 3 is already fixed: `patch_config` only drops the top-level keys of `[llm]` and keeps `[llm.passes.*]`.

## Plan
1. [ ] Fixed judge in `pass_replay`: `--judge <model>` (default: the replayed model, as today) judges every run;
   the table head names the judge; verified by a unit test of the argument parsing and a replay of delivery_router
   (<$0.01); makes the figures of two models comparable.
2. [ ] Split in the benchmark script: `--pass <name>=<model>` (repeatable) appends `[llm.passes.<name>]` with that
   model after the patched `[llm]`; the run directory records it; verified by `test_benchmark.rs` (patched TOML
   holds the sections, `[llm]` unchanged) and `just test-harness`.
3. [ ] Replays (paid, user's go): baselines with `deepseek-flash` then the same with `deepseek-v4-pro`, both judged
   by `deepseek-flash`, 3 runs each, on linkding and lobsters: domains and features (cents), use cases and rules
   on linkding (~$0.3 a series with flash, more with pro); figures, calls and tokens per pass in the issue and the ledger.
4. [ ] Final check (paid, user's go): one whole benchmark of linkding (3 runs, `--pass` with the chosen split)
   against the latest flash benchmark of the same commit, compared with `benchmark-table`; lobsters too if the
   replays show a gain on one application only.
5. [ ] Result: `# Results` in the issue, the README suggestion (`[llm.passes.domains]` example) and ADR 0023's
   "not yet measured" line kept, changed or dropped; ledger updated.

## Decisions
- Mode: autonomous
- 2026-10-27, user: no strong model in the solution for now, costs must stay reasonable. The plan above (strong `deepseek-v4-pro` on the framing passes) is dropped; what replaces it is pending.
- 2026-10-27, user: close the issue as abandoned, nothing measured, no code changed. Reopen it if a strong
  model (or a local model fast enough for a series) becomes an option; the plan above is the starting point.
- Base commit: <set at validation>
