# Tighten the quality benchmark: spread on linkding and trust in the judge

# Goal

Make the figures of the quality benchmark (`issues/done/quality_benchmark.md`) solid enough to decide
on the redesign of ADRs 0019 to 0023: a spread on `linkding`, an answer to which business-language
measure to trust, and a human check of the audited pairs.

# Findings

* The first benchmark of `delivery_router` has three runs per series; the one of `linkding` has one
  run per series, so no spread (one run of `linkding` takes 1h40 to 2h locally, about 4h per
  benchmark with the two series).
* Between runs of the same configuration the figures move as much as most of the changes one wants to
  detect (judged domain recall 50 to 100%, narratives in business language 22 to 71%).
* The two measures of business language disagree: the LLM judge says about 54% of the narratives
  (43 to 62% on `linkding`), the deterministic score says 88 to 93%.
* The judge's pairs were wrong 43% of the time for features of `delivery_router` and 14% on
  `linkding`; the audit was done by the same family of model, from descriptions and cited files.

# Approach

1. Run `linkding` three times per series in a terminal without the 2 h limit of background commands
   (`nohup just benchmark benchmark/linkding/reference.yaml --runs 3 > ...`, about 12 hours), then audit
   a sample of the judge's pairs as the skill describes.
2. Read a sample of narratives by hand (rated business and rated technical by each measure) and decide
   which measure to keep, or how to combine them; fix the other one or drop it.
3. Have a person read both `matches.yaml` files against `reference.yaml` and correct the pairs.
4. Consider a `--series` option of `benchmark.rs` to run only one series, and `--judge-model` to judge
   with another model than the one that generated the docs.

# Resources

* `.claude/skills/quality-benchmark/SKILL.md` (run and audit procedure)
* `issues/done/quality_benchmark.md` (decisions and baselines)
* `benchmark/delivery_router/`, `benchmark/linkding/`
* ADRs 0019 to 0023

# Hints

* A change smaller than the range between runs is noise: never conclude from one run.
* Only public repositories go into the benchmark; the clones and logs stay in `/tmp`.
