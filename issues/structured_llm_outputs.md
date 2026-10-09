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

# Tracking

1. [x] The provider sends a schema: optional `json_schema` on `CompletionRequest`, `response_format` in
   `openrouter.rs`, `llm.structured_output` setting (default `true`), fallback to the plain request when the
   server rejects it (remembered for the rest of the run).
2. [ ] `complete_json` sends the schema derived (`schemars`) from the raw answer type; derive it on every
   raw answer type, keep the lenient parsing and the retry.
3. [ ] Count retries and skipped units per pass in the end-of-run recap; write the measurement protocol
   (setting off vs on, same repo and model, several runs, narratives, truncations). The runs are made by
   the user.
4. [ ] Shorten the prompts that spell out the JSON shape, pass by pass, only if the measurement shows a gain.
5. [ ] Docs along the way (`update-docs`): ADR 0023 (or a new ADR), `CLAUDE.md`, `PLAN.md`, config doc;
   update `issues/use_cases_model_noise_warnings.md` and propose closing it only if justified.

## Decisions

* `llm.structured_output` defaults to `true`; a rejected request falls back to the plain one automatically
  (also the answer for DeepSeek if it lacks `json_schema`, no dedicated `json_object` mode).
* The smoke runs for the measurement are made by the user, not by the agent.
* `schemars` is accepted as a dependency.
* Order: 1, 2, 3, then 4 after the user's measurement.
