# Try again how the product brief and the evidence of each unit are injected, once the product is stable

# Goal

The benchmarks of `issues/done/product_brief.md` (deliverable 8) did not show that the brief improves the generated
docs, and the per-unit evidence made the features pass produce fewer features. The brief is on, the evidence
is off by default (`brief.evidence` in `retrodoc.toml`). Find out, with a stable pipeline and a steadier
measure, whether and how the global frame should be injected, then keep the best setting as the default.

# Findings (benchmarks of 2026-10-08, `delivery_router` and `linkding`, local `qwen3.6:35b-a3b` and DeepSeek)

* `delivery_router`, local model, 3 runs: use cases fully in business language 41% to 68% (hidden docs) and
  65% to 86% (shown), narratives and judged recall within the spread between runs, input tokens +42% and +22%.
* `delivery_router`, DeepSeek, 3 runs: nothing beyond the spread (the business-language figures were already
  at 96% to 100%), 9 and 12 features instead of 6, input tokens +43% and +95%.
* `linkding`, local model, 1 run: 42 features instead of 70, judged feature recall 10% instead of 75%.
  Replaying only the features pass on the same 8 domains: 56 features with nothing, 54 with the brief, 45 with
  the brief and the evidence. The rest of the drop comes from the domain clustering, which gave 8 domains
  instead of 11 (run-to-run variation, not measured).
* The first version of the evidence showed the origin of each extract (`spec/foo_spec.rb`); the model cited
  those as source files of the steps (13 references dropped by the grounding in the benchmark). Origins are
  no longer shown.
* The brief of a repository with few docs cites mostly test files (`delivery_router`), and its capabilities
  came back as bare strings in the first local runs.

# Approach

1. A steadier measure first: 3 runs or more per setting on `linkding`, the features pass replayed on fixed
   domains (the domain clustering varies too much between runs to compare settings; `pass_replay features`,
   baseline in the planned measures of the ledger, `CLAUDE.md`), recall and precision
   audited by hand rather than judged.
2. Settings to compare: no brief; brief in the system prompt instead of the head of the user prompt; brief
   only for domains and features; evidence limited to the use cases; fewer or longer extracts; extracts picked
   by the entry points of the feature instead of its names.
3. Separate the two effects: a smaller number of features is not worse per se (merged duplicates) and must be
   judged on the reference, not on the count.
4. Keep the best setting as the default and update ADR 0019.

# Resources

* `issues/done/product_brief.md` (Decisions and Measures), ADR 0019
* `crates/retrodoc-pipeline/src/brief/evidence.rs`, `features/mod.rs`, `use_cases/mod.rs`
* `benchmark/delivery_router/`, `benchmark/linkding/`, `skill:quality-benchmark`

# Hints

* Do it when the pipeline has stopped moving (behaviour-coverage domains, structured outputs, model per pass):
  every one of them changes the prompts the brief is injected in.
* `brief.evidence = true` in `retrodoc.toml` switches the evidence on for an experiment.
* A hosted model costs about $0.30 per `linkding` run; the local one takes 1 to 2 hours per run on the iGPU.

# Tracking

Plan: validated

## Understanding
Measure, on frozen linkding inputs and several runs per setting, whether the product brief and the per-unit evidence
improve the features and use cases, then keep the best setting as the default (ADR 0019, `specs/pipeline-settings.md`).
Out of scope: the `files` path prompts beyond sharing the setting, the domain clustering, local-model runs.
Means: `pass_replay` (surface path, deepseek-chat) instead of whole benchmarks; the replays today always load the
frozen brief and never the evidence, so the settings must first become replay options.

## Plan
1. [x] Replay settings: `pass_replay features|use-cases` takes `--brief off|head` and `--evidence` (extracts built from
   the signals of the `source_run` tree, as `generate` does); verified by replay tests with a recording provider (the
   prompt holds or lacks the brief and the extracts); ~300 lines.
2. [x] Measure 1, features on the frozen domains, 3 runs each: no brief, brief (today's default, also the G2 features
   baseline), brief + evidence; strict and judged recall/precision, then a hand audit of the features against
   `benchmark/linkding/reference.yaml` (merged duplicates counted as such); figures in the issue and the ledger.
   No code; ~$0.05.
3. [ ] Injection variants, kept only as experimental options of the pipeline: brief in the system message instead of
   the head of the user prompt; evidence searched with the entry points of the unit instead of its names; extract
   count and length; exposed in `pass_replay`; verified by prompt tests; ~400 lines. Dropped if measure 1 shows no
   effect of either the brief or the evidence beyond the spread.
4. [ ] Measure 2: the variants of 3 on features (3 runs each, cents), then the use cases replay with the two best
   settings plus no brief (3 runs each, ~$0.3 a series); hand audit as in 2; issue and ledger.
5. [ ] Default: the best setting becomes the default, the losing options and their code are removed, ADR 0019,
   `specs/pipeline-settings.md` (the `[brief]` keys) and `docs/crates/pipeline.md` updated; verified by the config
   and prompt tests of the kept setting; ~200 lines.

## Decisions
- Mode: autonomous
- Base commit: c85c157
- Path measured (user, 2026-10-28): the `surface` path by `pass_replay` only; the kept setting is applied to the
  `files` path too, whose features prompt builds the brief head the same way (`features/mod.rs`).
- Audit (user, 2026-10-28): the agent audits by hand one run per setting and every pair where the strict reading and
  the judge disagree; the other runs keep the judge's figures.
- Budget (user, 2026-10-28): one go for the paid runs of deliverables 2 and 4, deepseek-chat, at most $1.50 in all;
  more needs a new ask.

## Deliverable 1: replay settings

- done: `benchmark::Frame` (brief left out or not, evidence) taken by `replay_features` and `replay_use_cases`;
  `tree_evidence` builds the extracts from the signals of the `source_run` tree, its generated docs left out
  (`OutputConfig::generated_dirs` moved to `retrodoc-core` for it); `pass_replay --brief off|head --evidence`.
  Checked with no LLM on the linkding tree: the extracts come from its docs, changelog and tests, none from
  `docs/functional`.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass (no spec: a measurement tool) |
| docs | `docs/dev/commands.md`, `docs/crates/pipeline.md` |
| code review | low, see below |

- red: none traced (no spec); the frame test failed with the brief still in the prompt
  (`replay.rs:515`), the tree test with the generated page among the extracts (`frame.rs:97`).
- Code review: no findings
- not verified: a paid replay with the new flags (deliverable 2).
- decisions needed: none.
- Follow-up: `issues/rules_replay_without_evidence.md` (the rules replay gets no `rule_evidence`, `generate` gives it).

## Deliverable 2: measure 1, features on fixed domains (2026-10-28)

`pass_replay features` on frozen linkding (8 domains clustered from the surface by deepseek-chat, 80 entry
points), deepseek-chat, 3 runs a setting, at d600304; runs and kept features in the measures directory
`brief_injection_experiments-d600304/`. Hand audit of the 9 runs against the 24 features of
`benchmark/linkding/reference.yaml`: a reference feature is found when a generated feature names it, alone or
merged with others; a generated feature is out of the reference when it is plumbing (secret key, toasts,
database maintenance, task migration, health check, live reload, navigation and close page, asset removal).

| setting | features | found by hand (of 24) | precision by hand | judged recall | prompt tokens |
|---|---|---|---|---|---|
| no brief | 32 (28-36) | 69% (16-17) | 77% (75-81%) | 35% (17-67%) | 7.8k |
| brief (default) | 33 (33-34) | 74% (17-18) | 73% (73-74%) | 71% (71-71%) | 10.0k (+30%) |
| brief + evidence | 28 (26-30) | 75% (18-18) | 79% (77-81%) | 75% (67-79%) | 15.8k (+104%) |

* Every run places the 80 entry points; the settings differ by how they cut them, not by what they cover. 16 of
  the 24 reference features are found by every run; the 8 never found have no entry point of their own (sign in,
  PDF, browser extensions, feed reader, automatic tags, managed users) or only a partial one.
* The brief adds the notes (named in all 6 runs with the brief, in none without): one reference feature, from the
  capabilities of the brief. It also steadies the cut (33-34 features against 28-36).
* The evidence merges features (archive and share into the state actions, settings and custom CSS, backups),
  with no reference feature lost: the fewer features of `issues/done/product_brief.md` are merged duplicates
  here, not lost coverage. It names the Internet Archive snapshots in 3 runs of 3 (2 of 3 otherwise).
* The judge is not a measure here: no brief, runs 2 and 3, it proposed no pair (judged = strict, 17% and 21%)
  where run 1 got 67%. Its pairs were not kept (now saved, `RETRODOC_KEEP_FEATURES`).
* Every difference is one reference feature of 24 or a few points of precision, at the edge of the spread.

- done: measure 1 above; `pass_replay` keeps the features or domains of each run and the judgement
  (`RETRODOC_KEEP_FEATURES`), the replays save the judge's pairs in their work directory.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | pass (no spec) |
| docs | `docs/dev/commands.md` |
| code review | low, see below |

- red: none traced (no spec); the judge test failed with `the judgement is kept to audit its pairs`.
- Code review: no findings
- not verified: `judge-<n>.yaml` kept by a paid replay (the measure ran before the change).
- decisions needed: deliverable 3, see the stop below.
