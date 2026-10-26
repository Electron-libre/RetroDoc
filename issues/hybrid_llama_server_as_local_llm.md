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
