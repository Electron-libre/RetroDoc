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
  between runs), `issues/done/benchmark_passes_on_fixed_inputs.md`
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
2. [x] Hand-labelled sample of about 50 entry points (benchmark repositories), proposed by the agent and
   reviewed by the user.
3. [x] Replay on the saved runs: JSON answer vs logprob readout on the same model, then the small models.
   Table: unknown-feature placements, entry points placed, agreement with the baseline and the labels,
   agreement between two runs, time and tokens (prompt and completion) per question, and the gain in time and
   tokens over the JSON answer for the whole pass.
4. [-] Dropped (user decision: not worth it after step 3). Only if needed: SemIf itself with Qwen3.5-4B, to check that a 4B holds (licence of the weights first).
5. [x] Written decision in this issue: drop, keep for placement, or open an issue for a second provider trait.

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
* Step 2: sample in `benchmark/linkding/placement-sample.yaml` (50 entry points of a fresh linkding run, 34
  features, 116 entry points). The generated features overlap, so each entry point has a set of acceptable
  features. Labels written by the agent and checked against the code; the user does not know the application
  and trusts them. The pipeline's own placement falls in the sets 48 times out of 50; the 2 others are entry
  points the pass left without a feature.
* Performance and token cost are measured against the JSON answer at every step, not only at the end.

# Results (kept for the record and for a write-up)

## Question and protocol
Does a closed-answer decision model (SemIf, "read the probability of each lettered option from the logits of
one forward pass") place the entry points into features better, faster or cheaper than the JSON answer the
pipeline asks for today? Measured with `crates/retrodoc-pipeline/examples/placement_replay.rs`, replaying on a
fresh smoke run of linkding (commit `27b7303`, `--domains-from surface`, qwen3.6:35b-a3b; 34 features, 116 entry
points) the 50 entry points of `benchmark/linkding/placement-sample.yaml`. Labels: for each entry point, the set
of features it can acceptably go into (the generated features overlap), written by the agent and checked against
the code. A placement counts when it falls in the set. All figures on the same server (Ollama 0.40.4, AMD iGPU
instance, 32768 context, `reasoning_effort=none`, temperature 0), so times compare with each other, not with a
GPU.

Formats, on the same questions:
* `json`: one call with all 34 features and the 50 entry points, the system prompt, schema and provider of the
  pipeline (without the product brief). Two runs.
* `readout, bm25`: one question per entry point, the 16 features closest by BM25 as lettered options, the SemIf
  "direct" prompt (system message and JSON of evidence, criterion, options, copied from its `core.py`),
  `max_tokens=1`, softmax of the letters in the top 20 logprobs (a variant such as " A" counts for A).
* `readout, tournament`: the same over the 34 features: groups of 12, then a final round between the winners
  (4 questions per entry point), so no shortlist is needed beyond the 16 letters.
* `readout, tournament, options first`: same, the options before the evidence so the start of a question is
  shared, questions asked group by group so the server cache can serve it (SemIf's shared mode).
Steadiness: JSON, same answer in two runs; readout, same answer with the options in reverse order. The cost of
a readout is that of the questions it asks (the reversed ones only check the steadiness).

## Figures (second round; the first round is within a few points of it)
| format | model | placed | in an acceptable feature | unknown feature | steadiness | calls | seconds | prompt tokens (cached) | completion tokens |
|---|---|---|---|---|---|---|---|---|---|
| pipeline run (reference) | 35B | 48/50 | 96% | 0 | - | - | - | - | - |
| json, run 1 / run 2 | 35B | 50/50 | 96% / 100% | 0 | 84% | 1 | 41 / 22 | 3197 | 1157 / 913 |
| readout, bm25 | 35B | 50/50 | 92% | 0 | 76% | 50 | 73 | 31878 | 50 |
| readout, tournament | 35B | 50/50 | 96% | 0 | 26% | 200 | 183 | 81901 (6378) | 200 |
| readout, tournament, options first | 35B | 50/50 | 96% | 0 | 40% | 200 | 169 | 82066 (8869) | 200 |
| json, run 1 / run 2 | 9B | 50/50 | 88% / 94% | 0 | 92% | 1 | 57 / 46 | 3197 | 660 / 655 |
| readout, bm25 | 9B | 50/50 | 96% | 0 | 78% | 50 | 82 | 31878 | 50 |
| readout, tournament | 9B | 50/50 | 90% | 0 | 38% | 200 | 229 | 81840 (0) | 200 |
| readout, tournament, options first | 9B | 50/50 | 94% | 0 | 32% | 200 | 223 | 82007 (0) | 200 |

The BM25 shortlist of 16 held an acceptable feature for all 50 entry points. First round, same server: 35B json
98% / 100%, readout bm25 92%, tournament 96%; 9B json 92% / 84%, readout bm25 96%, tournament 90%. Two runs
of the same format differ by up to 8 points: the sample (50) cannot rank formats that are 4 to 6 points apart.

## What it says
* **Cost and speed: no gain here.** The JSON call puts the 34 features once and places 50 entry points; the
  readout puts its options again in every question: about 10 times the prompt tokens (4 times the 200 calls of
  the tournament) and 2 to 4 times the time. It generates 1 token instead of 650 to 1360, which does not make up
  for it on a local server where the prompt is the expensive part. Sharing the start of the question (options
  first) saved 8% of the time on the 35B (11% of the tokens read from the cache) and nothing on the 9B (no
  cache hit: the model does not reuse a prefix on this server).
* **Quality: the format does not change the 35B, it may help the 9B.** With the 35B the JSON answer is at least
  as good (96-100% against 92-96%). With the 9B the readout (94-96%) is above the JSON (88-94%) and close to the
  35B in JSON, within the noise of the sample.
* **Steadiness: the readout is sensitive to the order of the options.** 22 to 24% of the answers change when
  the order of 16 options is reversed, and 60 to 74% with the four-question tournament (each step can flip).
  The JSON answer between two runs is steadier (84 to 92%).
* **The failure that motivated the idea did not show.** The JSON answer cited an unknown feature once in 400
  placements here (0 for the 35B), against 95 drops on a large application with far more features. A
  closed-answer model cannot produce that failure, but this sample is too small to show the pipeline does.
* **Operational finding:** on an Ollama instance left at the default 4096 context, the 35B JSON answer is cut
  in the middle (3197 prompt + about 900 completion tokens): the readout never needs that room.

## Limits
50 entry points of one public repository; labels by the agent; lenient sets; one local server, two models; no
brief in the JSON prompt; no "none fits" option in the readout (the pipeline may leave an entry point out);
SemIf itself (Qwen3.5-4B, its backends, its calibration) was not run, only its prompt and readout were
reproduced against Ollama; the options were truncated to 160 characters of description as in the pipeline.

## Decision
Drop it. On this sample the closed-answer readout costs about 10 times the prompt tokens and 2 to 4 times the
time of the JSON answer for no better quality with the model of the pipeline, and it is sensitive to the order
of its options. No second provider trait, no change to the pipeline, SemIf not installed (step 4 dropped).
What stays: the example (`placement_replay`, which also replays the JSON answer on a labelled sample), the
labelled sample, and these figures. To reopen: a model small enough that the JSON answer fails (a 4B or less),
a feature list too long for one prompt (several hundred), or a server that bills a cached prefix at a fraction
(hosted APIs), where the options-first layout would count for more.
