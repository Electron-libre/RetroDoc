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
   `quality-benchmark`) against the baseline of the same pass (group G2 of the planned measures in the ledger of measures (see `CLAUDE.md`)):
   cents for domains and features, about $0.3 a series for use cases and rules, instead of 2 series x 3 full
   benchmark runs per split (about $1.8 a split). Item 1 is only needed for the final check.
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
