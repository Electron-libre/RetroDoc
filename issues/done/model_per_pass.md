# Allow a different model per pipeline pass

# Goal

Spend the strong model on the few calls that frame the documentation (product brief, domains, features)
and a cheap or local one on the many extraction calls (glossary, entry points, file reads, confidence).
Add an optional per-pass LLM section to `retrodoc.toml` (ADR 0023), the global `llm` section staying the
default.

# Findings

* One `LlmProvider` is built for the whole run (`crates/retrodoc-cli/src/commands/usage.rs`) from the
  single `llm` section of the config.
* `UsageTracker` already counts calls and tokens per pass and per model, so the recap can show the split.

# Approach

1. Config: `[llm.passes.<name>]` with the same keys as `[llm]` (model, base URL, key variable, timeout,
   concurrency), each optional and falling back to `[llm]`.
2. CLI: build one provider per distinct configuration, wrapped like today (heartbeat, usage), and hand each
   pass its provider.
3. Decide the pass names (the ones `set_pass` and `in_pass` already use).
4. Document a recommended split and measure it with `issues/quality_benchmark.md` (cost and quality).

# Resources

* ADR `0023`, ADR `0002`
* `crates/retrodoc-core/src/config.rs`, `crates/retrodoc-cli/src/commands/usage.rs`,
  `crates/retrodoc-llm/src/usage.rs`

# Hints

* A pass whose model changes must invalidate its cached results: include the model in its fingerprint.

# Tracking

1. [x] Config `[llm.passes.<name>]` in `retrodoc-core`: `PassLlmConfig`, `LlmConfig::for_pass`, unknown pass name rejected.
2. [x] One provider per distinct configuration in the CLI (`PassProviders::for_pass`), missing key reported with the pass name.
3. [x] Wire each pass (generate and the standalone commands) to its provider; the recap shows the split.
4. [x] Include the model in the fingerprint of each pass so a model change invalidates its cache.
5. [x] Docs: ADR 0023 "As built" or a new ADR, `CLAUDE.md`, `PLAN.md`, `docs/ARCHITECTURE.md`, recommended split and how to measure it.

## Decisions

* `[llm.passes.<name>]` accepts every key of `[llm]` (including `provider`, `reasoning_effort`,
  `structured_output`); `batch_chars` only applies to `repo-map`.
* An unknown pass name is a config error that lists the valid names.
* `sources` and `business-files` have their own pass key, falling back to `[llm]`.
* This issue delivers the mechanism and the recommended split; the real benchmark run with a hosted
  strong model is out of scope here.

## Notes

* The model of a pass is in the fingerprint of domains, features, use cases and actors, in the hash of
  each glossary and entry points file, and in the repo map cache (`model` field). The confidence pass
  keeps the model that scored in `fingerprints.json`; another model clears the scores and scores them
  again, within `--confidence-sample` if set (the rest stays unscored until a later run).
* `roles`, `sources`, `business-files` and `brief` are hand-editable and keep today's behavior (kept
  until `--force`).
* The first run after this change redoes every pass once (no model in the saved fingerprints).
