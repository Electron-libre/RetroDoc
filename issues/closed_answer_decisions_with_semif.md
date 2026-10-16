# Measure SemIf, a closed-answer decision model, on the placement of the entry points

# Goal
Some passes ask the LLM for a choice among a closed set (which feature holds this entry point, is this file
business or technical) and parse a generated JSON answer. A decision model reads the probability of each
allowed option from the logits in one forward pass and writes nothing, so it cannot cite an option that does
not exist, it is steadier between runs and it can be cheaper and faster. SemIf (formerly OpenJev, MIT) is the
candidate to try because it needs no model of its own: it reads the logits of a frozen Qwen3.5-4B. Find out
whether it beats the current JSON answer on the placement of the entry points, with figures, before any
decision to add it to the pipeline. This issue measures; it does not change the pipeline.

# Findings (desk research of 2026-10-15, nothing run yet)
* The sources are secondary (blogs, aggregators, model cards) and several projects share the name OpenJev: a
  27B model under CC BY-NC 4.0 (non-commercial, ruled out), a Qwen3.5-4B cross-encoder, a 26B diffusion
  server, and SemIf, said to be the former name of the MIT one. The README of SemIf was not read yet.
* Claimed by its authors, not verified: Qwen3.5-4B reaches 0.845 modal agreement against 0.883 for the
  proprietary Jev on a 102-row subset; 1.0 s against 5.3 s for the same 21 answers as generated JSON, with
  the same answer on 18 of 21; backends for CUDA, Apple silicon and llama.cpp on CPU.
* Other candidates were left out for now (Kev, Laya, NanoJev, jevlike): a weak score on many labels (Laya),
  a purpose other than classification, or no ready server for this machine.
* The placement pass (`features/from_surface/repair.rs`, `place_unclaimed`) is a closed choice: each entry
  point goes into one of the known feature slugs. It logged 95 times "placement in an unknown feature,
  dropped" and left 335 of 458 entry points without a feature on a large application
  (`issues/done/placement_into_unknown_features.md`). A closed-answer model cannot produce that failure.
* The machine (see its own hardware notes): a discrete NVIDIA GPU with 8 GB of VRAM served by Ollama with
  CUDA, and an AMD iGPU served by a second Ollama instance. A 4B model fits the NVIDIA GPU, even in BF16. The
  installed `llama.cpp-vulkan` did not load the recent Ollama GGUF files of the qwen3.5/3.6 family, so a
  llama.cpp backend would need a GGUF downloaded on its own, not the Ollama blobs.
* Not known: whether the Ollama in use returns logprobs; the latency per question on this machine; whether a
  frozen 4B places entry points as well as the 35B model of the smoke tests; what ground truth the
  placement has (`benchmark/linkding/reference.yaml` and `entry-points.yaml` give features and entry points
  by resource, not an entry point to feature mapping).

# Approach
1. Feasibility spike, before any code in the pipeline: read the README of SemIf; run it on the NVIDIA GPU
   with Qwen3.5-4B and time a question of a few options with the placement prompt's content; then try its
   llama.cpp backend on a separate GGUF. Stop here if neither answers in about a second.
2. Baseline, on the same inputs: (a) the JSON answer of the current model of the pass; (b) a small general
   model read by logprobs, if the local Ollama returns them, to separate what comes from the decision format
   and what comes from SemIf.
3. Measure on the saved runs, without touching the pipeline (a throwaway `examples/` binary in the style of
   `entry_points_replay`): replay the entry points left unplaced with their candidate features. Per
   candidate: placements in a feature that does not exist, entry points placed, agreement with the
   baseline and with a hand-labelled sample of about 50 entry points from the benchmark repositories,
   agreement between two runs, time and tokens per question.
4. Decide with the figures: drop it, keep it for placement alone, or open an issue for a second provider
   trait (probabilities over options) and for the other closed passes (roles, business or technical).

# Resources
* `crates/retrodoc-pipeline/src/features/from_surface/repair.rs`, `docs/crates/pipeline.md` (`features/`)
* `issues/done/placement_into_unknown_features.md`, `issues/done/sampling_temperature_for_steadier_runs.md` (spread
  between runs), `issues/benchmark_passes_on_fixed_inputs.md`
* `crates/retrodoc-pipeline/examples/entry_points_replay.rs`, `benchmark/linkding/`
* `skill:quality-benchmark`, `skill:smoke-test`, `docs/crates/llm.md` (`LlmProvider`)
* README of SemIf: https://raw.githubusercontent.com/TheoLeeCJ/SemIf/master/README.md

# Hints
* Check the licence of the exact weights downloaded: some OpenJev checkpoints are non-commercial.
* Saved runs live in the runs directory of the smoke-test skill, not in `/tmp`.
* Keep the baseline honest: a 4B model against a 35B one confuses size with format; say which is compared.
* The large application of the placement issue is confidential: describe it generically.

# Tracking
1. [x] Prototype of the SemIf readout in an `examples/` binary on the local Ollama (no SemIf install): SemIf
   direct prompt template, `max_tokens=1` with logprobs, softmax over the letters (variants such as ` A` folded
   in). Models: `qwen3.5:9b-q4_k_m`, `ministral-3:8b`, `qwen3.6:35b-a3b`. Measures: latency and tokens per
   question, rate of letters missing from the returned top logprobs, sensitivity to option order. Verified by
   unit tests of the softmax and the folding, and a run on a real question.
2. [ ] Hand-labelled sample of about 50 entry points (benchmark repositories), proposed by the agent and
   reviewed by the user.
3. [ ] Replay on the saved runs: JSON answer vs logprob readout on the same model, then the small models.
   Table: unknown-feature placements, entry points placed, agreement with the baseline and the labels,
   agreement between two runs, time and tokens (prompt and completion) per question, and the gain in time and
   tokens over the JSON answer for the whole pass.
4. [ ] Only if needed: SemIf itself with Qwen3.5-4B, to check that a 4B holds (licence of the weights first).
5. [ ] Written decision in this issue: drop, keep for placement, or open an issue for a second provider trait.

## Decisions
* Ollama 0.40.4 returns logprobs (`/v1/chat/completions` and `/api/generate`), so the logprob baseline is
  possible without another server.
* SemIf direct mode is a fixed prompt plus a softmax over the logits of the letters A..P (16 options at most),
  no training. The spike reimplements that readout against Ollama rather than installing SemIf; its source
  was read from `src/semif_phase1/{core,direct}.py`.
* Options are lettered, not slugs, to avoid multi-token labels. More than 16 candidates need a pre-selection
  or a tournament.
* Step 1 first figures (30 questions, 8 options, local Ollama, truth = the pipeline's own placement on one large
  saved run): qwen3.5:9b 83-87% agreement, 0.2-0.3 s and about 345 prompt tokens per call, 1 completion token;
  ministral-3:8b 77%, 7.5% of letters outside the top 20; qwen3.6:35b-a3b 90%, 1.7 s (includes a model load,
  to redo); 16 options with the 9B 83%, 1.7% of letters missing. Two samples of the same size differ by 4
  points: the sample is too small to rank models. Only the confidential run links features to entry points;
  the linkding runs predate that, so step 2 needs a fresh run on a public repository.
* Performance and token cost are measured against the JSON answer at every step, not only at the end.
