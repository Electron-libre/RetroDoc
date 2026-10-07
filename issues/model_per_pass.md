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
