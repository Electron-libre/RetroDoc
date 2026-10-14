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
