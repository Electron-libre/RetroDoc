# Fall back to json_object when a server refuses the JSON schema

# Goal

Give the servers that accept `response_format: {"type": "json_object"}` but not `json_schema` (DeepSeek, as
measured) the part of the benefit that remains: an answer that is valid JSON. Today the provider tries the
schema, gets a 400, sends the request again without any `response_format` and leaves the schema out for the
rest of the run (`issues/done/structured_llm_outputs.md`), so those servers get no help at all.

# Findings (probe of 2026-10-09, `just probe-schema`, one HTTP call per try)

* DeepSeek (`deepseek-chat`): `json_schema` strict is refused every time (HTTP 400, "This response_format
  type is unavailable now", 2 tries); `json_object` is accepted every time (3 tries out of 3) and the
  answer was valid JSON of the shape the prompt asked for.
* Ollama (local) and Gemini (`gemini-3.8-flash`) obey `json_schema`, so they do not need this.
* `json_object` carries no schema: the shape still has to be spelled out in the prompt, and DeepSeek-style
  servers require the word "JSON" in the messages. The prompts of the passes already describe the shape.
* Not measured: whether DeepSeek's plain answers (no `response_format`) are often unparseable. Without
  that figure the gain is unknown, and it may be small.
* `OpenRouterProvider` sends the schema first, then the plain request, and remembers the refusal in
  `schema_refused` (`crates/retrodoc-llm/src/openrouter.rs`).

# Approach

Measure first: count the unparseable answers of a DeepSeek `generate` run without `response_format` (the
recap shows `unparseable` and `skipped` per pass), on the same repository as before. If they are rare,
stop here and close this issue as not worth it.

If they are not rare:

1. Extend the fallback: a refused schema is sent again with `json_object` (the schema is dropped, the
   prompt carries the shape), and only a refusal of that too falls back to the plain request. Remember
   each refusal for the rest of the run, as `schema_refused` does.
2. Check that the messages contain the word "JSON" when `json_object` is sent (some servers reject the
   request otherwise); add a short line to the system prompt of the request if not, not to each pass.
3. Decide whether a setting names the mode (`llm.structured_output = "json_schema" | "json_object" |
   "off"`, `true` and `false` kept as they are) or whether the detection is enough. Propose, ask.
4. Extend `just probe-schema` if the provider's behavior is worth a regression check (it has
   `--json-object` already).
5. Measure again on DeepSeek: unparseable answers, skipped units, calls, before and after.

# Resources

* `issues/done/structured_llm_outputs.md` (measurement protocol, results), ADR `0023`, ADR `0024`
* `crates/retrodoc-llm/src/openrouter.rs` (`send`, `complete`, `schema_refused`),
  `crates/retrodoc-llm/src/types.rs` (`ResponseSchema`)
* `crates/retrodoc-llm/examples/schema_probe.rs` (`just probe-schema deepseek [--json-object]`)
* `.env` holds `DEEPSEEK_API_KEY`; a benchmark run on DeepSeek costs about $0.30 (ADR 0024)

# Hints

* A refusal can come from the model or the account and change over time ("unavailable now"): the
  fallback must stay automatic, never a hard error.
* `json_object` makes the server reject an answer that is not valid JSON, but says nothing about the
  fields: the lenient parsing and the retry stay.
* A 400 unrelated to `response_format` (prompt too long) must still not be blamed on the format: keep the
  rule "only if the next request works is the format refused".
