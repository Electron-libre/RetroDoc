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
  `issues/brief_injection_experiments.md`.
* `[pipeline] business_read` (on, ADR 0028): the switch exists to turn it off; no known case where that is
  wanted.
* `[pipeline] rules` (off, ADR 0021): about 42% of the input tokens of a run, rules still noisy
  (`issues/rules_deduplication_and_noise.md`).
* `[pipeline] domains_from`: `issues/surface_domains_default_decision.md` and `issues/single_pipeline_path.md`.
* `--confidence-sample N`, `--no-confidence`, `--max-files`, `llm.batch_chars`, `llm.structured_output`:
  options with a use, listed here so the review is complete.

# Approach
1. `body_lines`: measure again with `business-read` on (`entry_points_replay`, entity and attribute counts,
   three runs), and check on the use cases downstream whether the lost attributes matter (the vocabulary
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
