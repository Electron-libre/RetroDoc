# Use the business files in the entry points and actors passes

# Goal

The business-files pass (ADR 0025) now tells the glossary where the business lives. Two other passes still
choose their input by the old guess: the entry points read the files the roles pass classes `entrypoint`, and
the actors read the files whose name says "authorization" plus the glossary. Check whether they should read the
business list too, or whether the list should also say where the entry points and the authorization rules are,
so that a stack with an unusual layout is handled by the same pass.

# Findings (measures of 2026-10-08 on `delivery_router` and linkding, local Ollama, `qwen3.6:35b-a3b`)

* The `entrypoint` role has the same flaw as `model` had: on `delivery_router`, the whole `lib/**/*.rb` tree
  was classed `entrypoint` in 2 of 5 roles runs (the public API of a library) and a single file in the others.
* Nothing has been measured yet on the entry points and the actors with the business list: they were left
  unchanged on purpose in `issues/done/locate_business_files.md` (decision of step 2, "revisit only if the
  measure shows a gap").
* The business-files pass still lists views, serializers and API routes of a Django application in most
  runs (precision about 0.5 against a hand-made list); a second consumer would inherit that noise.

# Approach

First measure: on `delivery_router` and linkding, list by hand the entry points (routes, commands, public API)
and the actors, then compare with what `retrodoc entry-points` and `retrodoc actors` find over five runs.

Then, candidates (can be combined):

1. The entry points also read the business list, next to the `entrypoint` files.
2. The business-files pass answers a second list (where the entry points are), which replaces the `entrypoint`
   role as the input of the entry points pass.
3. The actors read the business list in addition to the authorization files.
4. Keep things as they are if the measure shows no gap.

# Resources

* `issues/done/locate_business_files.md`, ADR `0025`
* `crates/retrodoc-pipeline/src/business_files/`, `entry_points/`, `actors.rs`
* ADR `0020` (cover behaviours, not files)

# Hints

* The model answers differently each time: judge on several runs and on repositories of different shapes.
* Don't edit `roles.yaml` or `business-files.yaml` by hand to make a run pass (smoke test rule).
* Do not add a call per file.
