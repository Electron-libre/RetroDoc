# Try again how the product brief and the evidence of each unit are injected, once the product is stable

# Goal

The benchmarks of `issues/product_brief.md` (deliverable 8) did not show that the brief improves the generated
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
   domains (the domain clustering varies too much between runs to compare settings), recall and precision
   audited by hand rather than judged.
2. Settings to compare: no brief; brief in the system prompt instead of the head of the user prompt; brief
   only for domains and features; evidence limited to the use cases; fewer or longer extracts; extracts picked
   by the entry points of the feature instead of its names.
3. Separate the two effects: a smaller number of features is not worse per se (merged duplicates) and must be
   judged on the reference, not on the count.
4. Keep the best setting as the default and update ADR 0019.

# Resources

* `issues/product_brief.md` (Decisions and Measures), ADR 0019
* `crates/retrodoc-pipeline/src/brief/evidence.rs`, `features/mod.rs`, `use_cases/mod.rs`
* `benchmark/delivery_router/`, `benchmark/linkding/`, `skill:quality-benchmark`

# Hints

* Do it when the pipeline has stopped moving (behaviour-coverage domains, structured outputs, model per pass):
  every one of them changes the prompts the brief is injected in.
* `brief.evidence = true` in `retrodoc.toml` switches the evidence on for an experiment.
* A hosted model costs about $0.30 per `linkding` run; the local one takes 1 to 2 hours per run on the iGPU.
