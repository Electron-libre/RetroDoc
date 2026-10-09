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
2. [x] `complete_json` sends the schema derived (`schemars`) from the raw answer type; derive it on every
   raw answer type, keep the lenient parsing and the retry.
3. [x] Count retries and skipped units per pass in the end-of-run recap; write the measurement protocol
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

## Measurement protocol

The recap at the end of `generate` (and `.retrodoc/cache/usage.json`, last 20 runs, per pass:
`unparseable`, `skipped`, calls, tokens) gives the counts: an unparseable answer is one retry, or a skipped
unit when it was the second. The runs are made by the user.

1. Same repo (`delivery_router`), same model and server (`qwen3.6:35b-a3b`, `reasoning_effort = "none"`),
   same commit, `generate --force` each time, logs kept (`| tee run.log`).
2. Baseline: `structured_output = false` under `[llm]`, at least 3 runs. Then `structured_output = true`
   (the default), at least 3 runs.
3. Per run, note: the recap rows `unparseable` / `skipped` per pass, the total calls and tokens, the wall
   time, the number of `WARN` lines (`grep -c WARN run.log`), and the `response truncated
   (finish_reason=length)` warnings, kept apart: the schema does not remove them.
4. Look for `the server refused the JSON schema`: if present, the server does not support it and the
   second series measured nothing.
5. Narratives: run the `quality-benchmark` skill (`--judge`) on one run of each series and compare the
   narrative ratings and the business-language score, since constrained decoding can flatten text fields.
6. Report a table (before / after: unparseable, skipped, calls, tokens, time, WARN, narrative rating)
   in this issue. The runs differ at random, so read the trend over the runs, not one run.

## Measurement (2026-10-09, `delivery_router` HEAD, local Ollama, `qwen3.6:35b-a3b`, `reasoning_effort = "none"`)

Six `generate --force` runs, interleaved (off, on, off, on, off, on). The judge of the benchmark ran
afterwards on the same six runs without a schema, so only the generation differs.

| Run | Calls | Tokens in / out | Time | Unparseable (skipped) | WARN |
|---|---|---|---|---|---|
| off-1 | 26 | 25,831 / 11,065 | 6m22 | 1 (0) | 1 |
| off-2 | 37 | 41,924 / 18,732 | 11m11 | 4 (1) | 5 |
| off-3 | 27 | 24,686 / 11,908 | 7m10 | 1 (0) | 2 |
| on-1 | 29 | 21,032 / 14,544 | 7m59 | 0 | 4 |
| on-2 | 27 | 23,523 / 10,792 | 6m54 | 0 | 6 |
| on-3 | 24 | 21,994 / 10,531 | 5m26 | 0 | 1 |

* No `finish_reason=length`, no "server refused the JSON schema": Ollama accepted the schema.
* Unparseable answers: 6 without the schema (one unit skipped), 0 with it. Mean 30 calls / 8m14 without,
  27 calls / 6m46 with; the spread between runs (26 to 37 calls) is as large as the gap.
* The WARN lines do not drop (8 without, 11 with): with the schema the remaining ones are
  semantic (a use case citing an entry point that does not exist, a business path that does not exist),
  which a schema cannot prevent. This is the part of `use_cases_model_noise_warnings.md` that stays open
  (invented step and entry-point references).

Quality (benchmark judge, 3 runs per series): narratives in business language 34% (14 to 57) without the
schema, 49% (40 to 55) with it; use cases at score 1 54% (14 to 76) and 69% (53 to 100); domains the same.
No degradation of the text fields is visible. Judged feature recall and precision look worse with the
schema (38% / 50% against 54% / 87%), but this comes from one run (on-2, 0% matched): judging the same
generated docs three more times gave 38%, 50% and 62% recall (3, 4 and 5 pairs). It is noise of the judge,
not of the generation. The judge also failed to parse its own answer once (`null` where a string was
expected), with the schema turned off for it.

Limits: 3 runs per series, one repository, one model, the judge's pairs not audited, the "docs hidden"
series not run, and no hosted server (OpenRouter, Gemini, DeepSeek) tried.
