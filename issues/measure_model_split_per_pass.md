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
2. Pick the series to compare on a public repo of `benchmark/` (at least 3 runs each, the spread matters):
   one strong model for all, the same model with the split, a cheap model for all as the floor.
3. Compare recall and precision of domains and features, business-language score, confidence and cost
   (calls and tokens per pass and model from the recap) with `benchmark-table`.
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
