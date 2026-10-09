# 0023. Ask for structured outputs and allow one model per pass

Status: Accepted (refines 0002 and 0006, which stay in force)

## Context

Every JSON answer is asked for in the prompt only. The request sends `max_tokens` but no
`response_format`, so the lenient parsing and the retries of `response.rs` are the normal path, and the
remaining failures (cut or malformed objects, a field of the wrong type) are the bulk of the smoke-run
warnings (`issues/use_cases_model_noise_warnings.md`, `PLAN.md` §7).

The whole run uses one model. The passes are not alike: a few calls decide the frame of the whole
documentation (product brief, domains, features), many calls extract facts from code (glossary, entry
points, file reads, confidence). Paying the strong model for the second kind, or living with a weak one
for the first, are both waste.

## Decision

* Send a JSON schema with `response_format` (`json_schema`) for the passes that expect JSON, when the
  configured server supports it (Ollama's OpenAI-compatible endpoint does; on OpenRouter it depends on the
  model). Keep the lenient parsing as the fallback, and a setting to turn the schema off for a server that
  rejects it.
* Allow an optional model (and its settings) per pass in `retrodoc.toml`, for example under
  `[llm.passes.<name>]`; the global `llm` section stays the default. `UsageTracker` already counts per
  model, so the recap shows the split.

Rejected: a second provider implementation to get structured outputs (the OpenAI-compatible format
already carries them, ADR 0002).

### As built (`issues/structured_llm_outputs.md`)

* `complete_json` derives the schema of its answer type with `schemars` and sends it as a strict
  `response_format`. Strict mode wants every property required and no `default`, `oneOf` or extra
  property: `response_schema` rewrites the schema that way, so a `#[serde(default)]` field is still always
  asked for while the Rust type keeps accepting its absence.
* `llm.structured_output = false` never sends it. On the real OpenRouter endpoint the request also carries
  `require_parameters`, so the routing skips servers that would ignore the schema.
* A 400, 404 or 422 is sent again without the schema. Only if that works is the schema blamed and left out
  for the rest of the run (one warning); otherwise the original error is returned.
* The lenient parsing and the retry stay: a schema does not stop a cut answer, and a server may ignore it.

## Consequences

* Fewer retries and skipped units, so fewer calls and less lost documentation.
* The schemas must follow the Rust types of the raw answers; deriving them (for example with `schemars`)
  avoids drift.
* Prompts can be shorter: the shape no longer needs to be spelled out in prose.
* Follow-up issues: `issues/structured_llm_outputs.md`, `issues/model_per_pass.md`.
