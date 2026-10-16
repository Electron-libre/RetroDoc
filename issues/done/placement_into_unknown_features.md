# Stop the placement pass from dropping entry points into unknown features

# Goal
On a large application most entry points end up held by no feature: the surface run holds 123 entry points in
features and leaves 335 without one, and almost every resource sits in the "Uncategorized" domain. Find why
and make the features hold the behaviours of the application, so that the coverage the use cases report
(`issues/done/use_cases_record_their_entry_points.md`) is about the whole application.

# Findings (smoke run of 2026-10-14, a large Rails application, local `qwen3.6:35b-a3b`, `domains_from = "surface"`)
* The domains pass made two domains: one with 4 resources, and "Uncategorized" with 188 resources and 32
  entities.
* The features pass derived 5 features, holding 123 entry points.
* The placement pass (`features/from_surface/repair.rs`) logged 95 times "placement in an unknown feature,
  dropped"; 335 entry points stayed without a feature, and 1499 of 1661 source files are reached by no
  behaviour.
* The smoke verdict is FAIL because of these warnings (107 in all).
* Not known: whether the model names features that do not exist, or the ids/tags it is given are garbled; whether
  features are missing because "Uncategorized" is skipped by the features pass.

# Approach
1. Measure on the saved run, without a call: the features asked, the entry points shown to the placement call,
   and the feature names the answers cited.
2. Candidates, one at a time: (a) cite the features by a short tag, as the entry points are; (b) derive the
   features of the "Uncategorized" domain, or avoid putting most resources there (`domains_from = "contexts"`
   already splits by business context); (c) a second placement round for the dropped entry points.
3. Keep what raises the entry points held by a feature without lowering the quality of the features.

# Resources
* `crates/retrodoc-pipeline/src/features/from_surface.rs`, `features/from_surface/repair.rs`, `domains/`
* `docs/crates/pipeline.md` (`features/`, `behaviour_coverage.rs`), `issues/done/validate_on_large_application.md`
* The saved run of the smoke test (the runs directory of the smoke-test skill)

# Hints
* The application is confidential: no name, path or code of it in a committed file.
* The `contexts` mode was built for large applications; check first whether the problem is in the `surface` mode only.

# Tracking

1. [x] Measure without a call (`features_by_domain_measure` example): entry points, features and share held per domain, unplaced entry points, unknown cited slugs when replayable.
2. [x] Derive the features of the "Uncategorized" domain too (sliced), adjusting `place_per_domain` and `retain_other_domains`.
3. [-] (dropped: no unknown feature slug in the step 5 run) Cite the features by a short tag in the placement call (only if unknown slugs remain after 2).
4. [-] (dropped: no entry point left out in the step 5 run) Second placement round for the entry points still left out (only if they remain significant after 2).
5. [x] Smoke test with the local Ollama on the same application, compare the entry points held before and after.

## Decisions
* First lever: derive features for the "Uncategorized" domain (the features pass skips it), not switching to `domains_from = "contexts"`.
* The measure is a no-LLM example, run on the saved run and on the `contexts` runs to tell whether the problem is specific to the `surface` mode.
* A full run with the local Ollama closes the issue to quantify the gain (indicative: one run, one local model).
* Step 1 result: on the saved run, "Uncategorized" holds 444 of 458 entry points and has 0 features (only 109 are held, through the placement into features of another domain). No other saved run has a `features.yaml`, so the `contexts` comparison and the cited unknown slugs wait for the step 5 run.
* Step 2 done: `build_features_from_surface` no longer skips the "Uncategorized" domain (the `contexts` mode never builds one). Steps 3 and 4 wait for the step 5 run to say whether unknown slugs or unplaced entry points remain.
* Step 5 result (a large Rails application, local `qwen3.6:35b-a3b`, `domains_from = "surface"`, one run that rebuilt everything: 4h19m, 279 calls): 852 entry points, all held by a feature (before: 123 of 458), 85 features in 7 domains (before: 5 in 1), the "Uncategorized" domain holds 21 features. No "placement in an unknown feature" warning; 12 entry points no feature claimed were placed by the placement call. The rerun makes no call. Caveat: the run also redid the entry points and the domains (2 domains before, 7 now), so the gain is not the filter alone.
* The smoke verdict is still not CONCLUSIVE: 15 warnings (cited resources and entity the surface does not know, one truncated answer, one unparseable use cases answer retried) and the rerun rewrites 3 files (a `generated_at` timestamp). Both are outside this issue.
* Smoke harness gap found: the default HTTP timeout (2 minutes) cuts the entry points call of a large routes file on the local model and logs it as a network failure; the run only went through with `llm.timeout_secs = 900` in the clone's `retrodoc.toml`. `smoke.rs` does not set it.
