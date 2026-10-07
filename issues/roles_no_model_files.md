# Keep the surface from going empty when the roles pass finds no `model` file

# Goal

Understand why the roles pass sometimes classifies no file as `model` on a small Ruby repo whose domain classes
(`Order`, `Customer`, `Rider`, `Restaurant`) live in plain `lib/**/*.rb` files, and make the surface (glossary
entities, then the clustering prompt) not depend on that luck. With no `model` file the glossary has no entity,
the surface is empty and the clustering loses the business hints it is meant to use (ADR 0008).

# Findings (smoke runs of 2026-10-07 on `delivery_router`, local Ollama, `qwen3.6:35b-a3b`)

* The first smoke run of the earlier issue had entities in its surface (`Order`, `Customer`, ...). In the next
  runs the roles pass answered rules such as `lib/**/*.rb -> logic` and `lib/*gem_name*.rb -> entrypoint` and no
  `model` rule: `glossary.yaml` had `models: {}` and `retrodoc surface` printed "0 entit(ies)".
* Another run answered a rule with an unknown role for `lib/**/*.rb`; the rule was ignored with a `WARN`, and
  that rule came back on the second run because `roles.yaml` is reused.
* The domain classes are plain Ruby objects, not ActiveRecord models, so "a data model" is a judgment call for
  the model, and the repo has no `models/` folder to point at.
* The clustering stayed complete in those runs (the repo is small). The cost is a weaker input on a larger repo
  of the same style, not a failure here.

# Approach

First measure: replay the roles prompt on this repo N times and count how often a `model` rule appears, and
what the other answers look like. Then, candidates (can be combined):

1. Prompt: say that a class holding domain data and rules is a `model` even outside an ORM, and that a repo
   without a `models/` folder still has them.
2. Fall back when the glossary is empty on a repo with source files: ask for entities from the files the roles
   call classed as `logic` (bounded by a budget), instead of leaving the surface empty.
3. Make the empty surface visible: one `info` line "no model file, the clustering gets no business hints".

# Resources

* `crates/retrodoc-pipeline/src/roles/` (prompt, `RoleRules::classify`, unknown role handling)
* `crates/retrodoc-pipeline/src/glossary/` and `surface.rs`
* ADR `0008` (surface extraction), `.claude/skills/smoke-test/`

# Hints

* The model answers differently each time: judge a change on several runs, not one.
* Don't edit `roles.yaml` by hand to make a run pass (smoke test rule); `retrodoc roles --force` regenerates it.
* The generated artifacts live in the clone under `/tmp`, never commit them.
