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

# Tracking

1. [x] Measure DeepSeek as it behaves today (no code): `generate --force --no-confidence` on
   `delivery_router`, one run first, then up to two more if the first costs under $0.10; stop at 3 runs
   or $0.30 in total. Table in this issue; decision: close as not worth it, or go on to 2.
2. [-] Fallback chain (not done: step 1 found no gain) `json_schema` -> `json_object` -> plain (only if 1 justifies it): test with the fake
   server first, then the second refusal flag and the word "JSON" in the request.
3. [-] Docs (ADR 0023, `CLAUDE.md`) and a new DeepSeek run to compare before / after (only if 2).

## Decisions

* The agent runs the measurements itself, capped at $0.30 in total without asking again.
* No new setting: `llm.structured_output` stays a boolean, the detection is automatic.
* Go on past step 1 only if at least one run in three skips a unit, or there is more than one
  unparseable answer per run on average.

## Measurement (2026-10-09, `delivery_router` HEAD in a clone, `deepseek-chat`, `generate --force --no-confidence`, `concurrency = 4`)

| Run | Calls | Tokens in / out | Time | Unparseable (skipped) | WARN |
|---|---|---|---|---|---|
| 1 | 20 | 30,330 / 16,042 | 1m06 | 0 | 1 |
| 2 | 21 | 26,398 / 12,743 | 59s | 0 | 1 |
| 3 | 20 | 23,430 / 11,132 | 51s | 0 | 1 |

* The only `WARN` of each run is "the server refused the JSON schema": one wasted call per run, then plain
  requests. The recap shows no `unparseable` and no `skipped` row, in any pass.
* About 80k tokens in and 40k out for the three runs together.
* Below the threshold of the decisions (a skipped unit in one run out of three, or more than one
  unparseable answer per run): `json_object` would fix a problem DeepSeek does not have on this
  repository. Limits: one small repository, 3 runs, no confidence pass; `linkding` (larger answers, more
  units) was not run.

## Conclusion

Not worth it for now: deliverables 2 and 3 are left out. Reopen if a larger repository on DeepSeek shows
unparseable answers or skipped units.
