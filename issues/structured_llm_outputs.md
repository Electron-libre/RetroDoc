# Ask the server for schema-constrained JSON answers

# Goal

Make malformed JSON answers rare instead of repairing them. Send a JSON schema with `response_format` for
the passes that expect JSON (ADR 0023), keep the lenient parsing as the fallback, and measure the drop of
retries and skipped units.

# Findings

* The request sends `max_tokens` but no `response_format` (`crates/retrodoc-llm/src/openrouter.rs`).
* Smoke runs of 2026-10-07 on `delivery_router` (local Ollama, `qwen3.6:35b-a3b`) warned on unparseable
  use case answers and confidence verdicts (`EOF while parsing an object`, `expected value`), see
  `issues/use_cases_model_noise_warnings.md`.

# Approach

1. Check what each target accepts: Ollama's OpenAI-compatible endpoint, OpenRouter (per model, with its
   `require_parameters` routing option), the Gemini compatible endpoint.
2. Extend the completion request with an optional schema; derive the schemas from the raw answer types
   (for example with `schemars`) so they can't drift.
3. Use it in `complete_json`; a setting turns it off for a server that rejects it, and a rejected request
   falls back to the plain one.
4. Measure retries, skipped units and calls over several smoke runs, before and after.
5. Shorten the prompts that spell out the JSON shape in prose, once the schema carries it.

# Resources

* ADR `0023`, ADR `0006`, ADR `0012`
* `crates/retrodoc-llm/src/types.rs`, `openrouter.rs`, `crates/retrodoc-pipeline/src/response.rs`
* `issues/use_cases_model_noise_warnings.md` (may be closed by this one)

# Hints

* A schema does not prevent a cut answer (`finish_reason=length`): truncation stays a separate cause.
* Constrained decoding can lower the quality of some models' text fields: check the narratives too.
