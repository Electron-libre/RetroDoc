# Run the smoke test and the benchmark on the hybrid llama-server

# Goal
The machine now has a second way to serve the local model: a llama.cpp `llama-server` that keeps attention and
KV cache on the NVIDIA card and the MoE experts in RAM. It serves the same `qwen3.6:35b-a3b` GGUF as the Ollama
iGPU instance about twice as fast in generation, with a 128k context. RetroDoc's local tooling (`just smoke`,
`just benchmark`, `just probe-schema`) only knows the Ollama instance. Check that RetroDoc works end to end
against this server and make it a first-class local target, so local runs get faster without a hosted provider.

# Findings (machine notes of 2026-10-26, no RetroDoc run yet)
* Server: on-demand user systemd service `llama-hybride`, stopped by default, OpenAI-compatible API on
  `http://127.0.0.1:11437/v1`, model alias `qwen3.6:35b-a3b-hybride`, context 131,072, `-np 1` (one slot),
  `--jinja`, flash attention, KV cache in `q8_0`, llama.cpp build b11898.
* Measured by the machine benchmarks (not by RetroDoc): generation about 54 tokens/s against about 28 on the
  Ollama iGPU instance; prompt processing about 406 tokens/s against about 505.
* Starting the service unloads the models of the Ollama NVIDIA instance (`:11434`). After 10 minutes without a
  request it unloads the model and reloads it in about 1.5 s on the next one; `/health` stays 200 meanwhile.
  If the NVIDIA Ollama instance took the card in between, the reload can run out of VRAM.
* llama.cpp server source (read, not run): `reasoning_effort: "none"` sets `enable_thinking = false`, and
  `response_format` with `type: json_schema` is turned into a grammar. So the two request fields RetroDoc
  depends on for this thinking model should be honoured; not verified on the running server.
* Not known: whether it returns `usage` in the shape `usage::parse` reads; whether an answer cut by the
  context or `max_tokens` reports `finish_reason=length`; how a refused or unsupported schema shows up; whether
  the speed gain holds on RetroDoc's prompts, which are long (prompt processing is the slower side here).
* `skill:smoke-test` already documents a manual run against it (`--model` and `--base-url`); `smoke.rs`,
  `benchmark.rs` and the `ollama` preset of `schema_probe` hard-code the Ollama iGPU endpoint.

* Left by `issues/done/reconciliation_judge_sees_whole_sections.md` (2026-10-26): the before / after comparison of
  the reconciliation judge on the local model, paused until this switch (about 3.5 min per judge batch on the
  iGPU). The labelled sample, the old and new binaries and the replay tool (`judge-sample`, `score`) are in
  `~/retrodoc-runs/reconcile-sections` and `~/retrodoc-runs/reconcile-replay`; `labels-local.yaml` holds the 17
  answers to compare (13 batches per version).
# Approach
1. Probe first: with the service started (ask the user, it takes the NVIDIA card), run
   `just probe-schema` against it with a `--base-url`/`--model` override, strict schema then `--json-object`;
   note reasoning off or not, schema obeyed, `usage` present, `finish_reason` on a forced cut.
2. Smoke test on the default smoke repository against it, then the same against the Ollama iGPU instance:
   compare verdict, duration and warnings.
3. Depending on 1 and 2, candidate changes (can be combined):
   a. a named local target in `smoke.rs`, `benchmark.rs` and `schema_probe` (e.g. `--local hybrid`) instead of
      typing the URL and alias, with the server check naming the service to start when nothing answers;
   b. whatever request difference the probe reveals, in `OpenRouterProvider` and not only in the scripts;
   c. local defaults that fit one slot (`concurrency = 1`) and the larger context (bigger batches), measured
      before being kept;
   d. a benchmark series on it to know whether the generated docs differ from the iGPU instance (same weights,
      different runtime and sampling defaults).

# Resources
* `.claude/skills/smoke-test/SKILL.md` (section on the hybrid llama-server) and `smoke.rs`.
* `.claude/skills/quality-benchmark/SKILL.md` and `benchmark.rs`.
* `crates/retrodoc-llm/examples/schema_probe.rs`, `crates/retrodoc-llm/src/openrouter.rs`, `docs/crates/llm.md`.
* ADR 0002 (one provider, any OpenAI-compatible server), ADR 0023 (structured outputs).
* `issues/done/smoke_http_timeout_for_the_local_model.md`, `issues/done/structured_llm_outputs.md`.
* The machine's own inference notes (see the global agent instructions) for the service and its figures.

# Hints
* Never stop it with `pkill -x llama-server`: Ollama's runners have the same process name. Use
  `systemctl --user stop llama-hybride`.
* Don't run it together with another smoke test or benchmark: one slot, and the NVIDIA card is shared.
* The faster generation can be eaten by the slower prompt processing on RetroDoc's long prompts: compare
  wall-clock durations of whole runs, not tokens per second.

# Tracking

Plan: validated

## Understanding
Make the hybrid llama-server (`:11437`) a checked, first-class local target of `just probe-schema`, `just smoke`
and `just benchmark`, and know from whole-run wall clock whether it beats the Ollama iGPU instance on RetroDoc.
Out of scope: changing the default local target before the measures say so; any hosted provider.

## Plan
1. [x] Probe reports what the issue lists as unknown: `schema_probe` gains a `hybrid` preset, a `--max-tokens N`
   flag and prints `finish_reason` and the parsed `usage`; then run it on the server (strict schema,
   `--json-object`, forced cut) and record the answers here; verified by unit tests of the new output and the runs.
2. [x] Named local target: `--local igpu|hybrid` in `smoke.rs` and `benchmark.rs` (default `igpu`, explicit
   `--model`/`--base-url` still win), the "no server answers" message names `systemctl --user start llama-hybride`
   for the hybrid target; skills updated; verified by `just test-harness` tests.
3. [x] Smoke comparison on `delivery_router`: hybrid then iGPU, one at a time; record verdict, wall clock,
   warnings here; verified by the two `just smoke` verdicts.
4. [x] Only if 1 or 3 shows a request or response difference: fix it in `OpenRouterProvider` (spec and test
   first); otherwise noted as not needed.
5. [x] Only if 3 shows queued calls cost time: local defaults for one slot (`concurrency = 1`) and the larger
   context (bigger batches), measured by a second hybrid smoke and kept only if faster.
6. [ ] Benchmark series on the hybrid target (`just benchmark`), compared with the last iGPU series; record
   whether the docs differ.

## Deliverable 1: probe of the hybrid server (2026-10-26, llama.cpp b11898)
| check | answer |
|---|---|
| strict `json_schema` | OBEYED, 4.8 s, `finish_reason=stop`, usage `prompt_tokens=53 completion_tokens=133` |
| `json_object` | accepted, valid JSON, but the model wrote a float for `population_millions`: the model's answer, not the mode (RetroDoc sends the strict schema) |
| forced cut (`--max-tokens 8`), both modes | `finish_reason=length`, usage present (`completion_tokens=8`): the provider's truncation check works |
| `reasoning_effort: "none"` | honoured: no `reasoning_content`, 2 tokens for "Say hi"; without it, 200 tokens of `reasoning_content` and an empty `content` |
| `usage` shape | `prompt_tokens`, `completion_tokens`, `total_tokens`, `prompt_tokens_details.cached_tokens`: the shape `usage::parse` reads, prompt cache included |

No request difference so far: deliverable 4 hangs on the smoke test only.

- done: `schema_probe` has a `hybrid` preset and `--max-tokens N`, and prints `finish_reason` and the `usage` counters after the verdict; `docs/dev/commands.md` updated.
- red: `cannot find function details in this scope` (the three new tests, before the code).
- checks: `just check` ok, `just test-harness` ok.
- Code review (low): `--max-tokens 0` was accepted against its own message; fixed (zero rejected).
- not verified: how the server shows a refused or unsupported schema (no schema RetroDoc sends was refused).

## Deliverable 2: `--local igpu|hybrid`
- done: `just smoke` and `just benchmark` take `--local igpu|hybrid` (default `igpu`); `--model`/`--base-url`
  still win; when nothing answers, the error names the command that starts the target's server; `--local`
  with `--provider deepseek` is an error; both skills updated.
- checks: `just check` ok, `just test-harness` ok.
- red: `error: usage: smoke.rs run <repo> | …` (no `llm-target`), and `no LLM server answers on 127.0.0.1:1; start it or pass --base-url` (no service named).
- Code review (low): no findings.
- not verified: a real run with `--local hybrid` (deliverable 3).

## Deliverable 3: smoke comparison on `delivery_router` (2026-10-26, one run each, hybrid first)
| | hybrid (`--local hybrid`) | iGPU (`--local igpu`) |
|---|---|---|
| verdict | CONCLUSIVE, no warning | CONCLUSIVE, no warning |
| wall clock of `just smoke` | 261 s | 595 s |
| LLM calls, tokens in / out | 23, 21,581 / 10,160 | 27, 30,776 / 17,857 |
| features, use cases, files | 4, 11, 19 | 6, 22, 32 |
| grounding drops (INFO) | 8 | 15 |

Same calls, same prompts (within a few tokens), wall per pass: brief 10 / 14 s, business-read 19 / 31 s,
repo-map 19 / 25 s, domains 6 / 13 s, features 11 / 18 s. The use-cases pass is 34 s a call on the hybrid against
68 s on the iGPU. The prompts are short on this repository (at most 2.3k tokens a call outside the use cases), so
the slower prompt processing does not show; it is not measured on a large repository.

The two runs did not find the same features (4 against 6), so their outputs, and the use cases that follow, are
not comparable: one run each says nothing on the spread. Deliverable 6 measures quality.

Artifacts (logs, reports, `usage.json` of both): `~/retrodoc-runs/hybrid-vs-igpu`.

## Deliverables 4 and 5: not needed
- 4: the probe and the smoke run show no request or response difference from Ollama (schema obeyed, reasoning
  off, `usage` and `finish_reason` read, no warning): nothing to change in `OpenRouterProvider`.
- 5: nothing to measure on this repository: its passes send at most 6 calls, which the one slot serves one
  after the other without any timeout or retry, and no batch comes near the 32k context of the iGPU instance.
  Local defaults stay as they are; to revisit only on a large repository where calls queue past the timeout.

## Decisions
- Mode: autonomous
- Base commit: 72c5081
- Server: use the running `llama-hybride` service throughout and leave it running at the end (user, 2026-10-26).
- The paused before / after comparison of the reconciliation judge stays out of this issue and is not tracked elsewhere (user, 2026-10-26).
- Benchmark series of deliverable 6: one run (`--runs 1`) (user, 2026-10-26).
