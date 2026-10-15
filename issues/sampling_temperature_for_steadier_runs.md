# Set the sampling temperature of the LLM requests to make runs steadier

# Goal
Two runs on the same input give different entry points, domains and features, and the spread between runs
is as large as most effects the benchmark tries to measure. The requests leave the sampling to the server's
default. Let the configuration set it, measure what a low temperature does to the spread and to the
quality, and choose the default with the figures. A steadier run also means fewer runs per benchmark.

# Findings (project review of 2026-10-12)
* `ApiRequest` (`crates/retrodoc-llm/src/openrouter.rs`) sends `model`, `messages`, `max_tokens`,
  `reasoning_effort`, `response_format` and `provider`: no `temperature`, no `seed`. `LlmConfig` has no key
  for them.
* Spread already measured: 13 to 21 entry points on `delivery_router` with deepseek
  (`issues/surface_domains_default_decision.md`), 64 against 96 entry points between two identical local runs
  on linkding (`issues/done/tree_sitter_structure.md`), 8 against 11 domains between two
  linkding runs, put down to run-to-run variation without a measure (`issues/brief_injection_experiments.md`), feature recall of three `surface` runs from 12% to 88%
  (`issues/done/behaviour_coverage_domains.md`).
* "Never conclude from one run" (`issues/done/quality_benchmark.md`): every comparison costs 3 runs or more
  per setting.
* Not known: the default temperature each server applies, and whether a strict JSON schema already narrows
  the answers.

# Approach
1. Measure first, on the passes that can be replayed alone and without a judge: `entry-points` on linkding
   and `delivery_router` (`entry_points_replay`), 5 runs at the server default and 5 at temperature 0; count
   the entries, the groups of the reference found, and how many entries all the runs share.
2. Add `temperature` (and `seed` if the measure wants it) to `[llm]` and `[llm.passes.<name>]`, sent only
   when set; it joins what `PassProviders` compares.
3. Candidates for the default: unset (as now); 0 for the extraction passes (roles, sources, business files,
   glossary, entry points, actors, confidence) and the server default for the writing ones (brief, domains,
   features, use cases, rules); 0 everywhere.
4. Check the quality of the writing passes with the benchmark before changing their default (a low
   temperature can make narratives poorer).
5. Decide whether the temperature is part of the fingerprints (a change would redo the pass).

# Resources
* `crates/retrodoc-llm/src/openrouter.rs`, `types.rs`; `crates/retrodoc-core/src/config.rs`
  (`LlmConfig`, `PassLlmConfig`, `for_pass`)
* `crates/retrodoc-pipeline/examples/entry_points_replay.rs`, `benchmark/`
* ADR 0023; `just probe-schema`

# Hints
* A server may refuse or ignore the parameter: handle it as `response_format` is (a refusal is sent again
  without it), and check each provider with one call.
* Temperature 0 is not determinism on a hosted model: measure the spread, do not assume it is gone.
* A reasoning model may ignore the temperature.

# Tracking
1. [x] Send `temperature` from the config: `LlmConfig` / `PassLlmConfig` / `for_pass`, `ApiRequest` (only when set, sent again without it when the server refuses), part of what `PassProviders` compares; one `probe-schema` call per provider.
2. [x] Measure the spread of `entry-points` on linkding and `delivery_router`: 5 runs at the server default against 5 at temperature 0, local Ollama first, then one deepseek series; table in the issue. `seed` only if the figures call for it (separate deliverable).
3. [x] Choose the default for the extraction passes with the figures, decide whether the temperature is part of the fingerprints, ADR if the default changes, docs.
4. [ ] Writing passes (brief, domains, features, use cases, rules): quality benchmark at a low temperature against the server default before changing their default.

## Decisions
* Measure locally (Ollama), then confirm with one deepseek series.
* `seed` is added only if the measure shows it narrows the spread beyond temperature 0.
* A reasoning model that ignores the temperature is documented, not worked around.
* Deliverable 4 stays in this issue.
* Deliverable 1 review follow-ups for deliverable 3: the temperature is not in any fingerprint yet; an out-of-range value is a refusal (400) and is silently dropped with a warning (consider validating); a refusal not caused by the parameter can switch it off for the run (same risk as the schema).

## Measure of deliverable 2 (2026-10-15)
`entry-points` alone, 5 runs per cell, same `roles`, `brief` and `business-files` (computed once per repo), no
cache between runs. Local = qwen3.6:35b-a3b via Ollama, `reasoning_effort=none`; deepseek = `deepseek-chat`.
"Shared" = entries (file + name) found by all 5 runs / by any run; "pairwise" = mean Jaccard of two runs.
Groups = share of the reference groups found (`entry_points_replay`), mean (range).

| Repo, server | Setting | Entries (range) | Groups found | Shared by all 5 | Pairwise |
|---|---|---|---|---|---|
| linkding, local | server default | 124 (105-142) | 86% (79-93) | 9 of 315 (3%) | 25% |
| linkding, local | temperature 0 | 130 (125-134) | 86% (86-86) | 102 of 164 (62%) | 81% |
| linkding, deepseek | server default | 165 (150-187) | 87% (86-93) | 55 of 350 (16%) | 42% |
| linkding, deepseek | temperature 0 | 160 (156-163) | 86% (86-86) | 140 of 184 (76%) | 88% |
| delivery_router, local | server default | 11.6 (5-20) | 65% (62-75) | 1 of 34 (3%) | 37% |
| delivery_router, local | temperature 0 | 17.4 (13-23) | 78% (62-100) | 12 of 23 (52%) | 72% |
| delivery_router, deepseek | server default | 15.2 (14-16) | 57% (38-62) | 1 of 42 (2%) | 25% |
| delivery_router, deepseek | temperature 0 | 16 (16-16) | 62% (62-62) | 16 of 16 (100%) | 100% |

Reading:
* Temperature 0 narrows the spread a lot on both servers and both repos: the entries shared by all runs go
  from 2-16% to 52-100%, the range of the number of entries shrinks (linkding local 105-142 to 125-134,
  deepseek 150-187 to 156-163), and the group recall stops swinging (linkding 79-93% to a flat 86%).
* The mean recall does not rise or fall in a consistent way (linkding unchanged, delivery_router +13 points
  local and +5 deepseek, inside the old range): the gain is steadiness, not quality. Precision of the
  entries inside the reference files moves little (linkding 74 to 72% local, 55 to 60% deepseek).
* It is not determinism: linkding local still differs by 19% between two runs (pairwise 81%), deepseek by 12%.
  `seed` was not tried; the figures do not call for it yet (deepseek delivery_router is already identical).
* Both servers accepted `temperature: 0` and no run logged a refusal.
* Runs are in `~/retrodoc-runs/eps-temp/{default,t0,ds-default,ds-t0}/`.
* Deliverable 3: default 0 for `EXTRACTION_PASSES` (sources, roles, business-files, glossary, entry-points, actors,
  confidence) when nothing is set, writing passes unchanged; the temperature stays out of the fingerprints
  (including it redoes the glossary, entry points and actors, and through them most of the run); ADR 0030.
  Known limits: no way to ask for the server's default on an extraction pass; `contexts` not measured, left
  out; only `entry-points` was measured.
