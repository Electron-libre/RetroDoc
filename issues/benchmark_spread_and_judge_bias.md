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

* Cost of one `linkding` run (generation and judge), measured on the local model: about 615 calls,
  1.16 M tokens in and 365 k out for the two series, about 3h45; three runs per series would be about
  3.5 M in, 1.1 M out and 11 hours. The use cases and confidence passes are 69% of the tokens in, 80%
  of the tokens out and 78% of the time; everything up to the features pass is about 30% of the tokens
  and 22% of the time (about 30 minutes per series). The judge costs 12 calls, 2 minutes.
* The domain and feature figures only depend on the passes up to `features`; only the narrative
  measures need the use cases.
* Rough price on a hosted model (DeepSeek V4.1 Flash, from third-party pages, not checked on the
  official one): $0.15 to $0.30 per million tokens in and $0.60 to $1.20 out, so about $0.4 to $0.8
  per `linkding` run and $1.2 to $2.4 for three runs per series, before counting caching. Tokens
  were counted on another model: a different tokenizer, or a model that reasons, changes them.

* First results on a hosted model (`deepseek-chat` through the DeepSeek API, `retrodoc.toml` with
  `provider = "deepseek"`, `--no-confidence`, 3 runs per series). The model followed the JSON formats on all
  12 runs, no failure. `delivery_router`: about 45 s and 29 k tokens per run, $0.12 for the seven runs
  and the connection test. `linkding`: 222 to 283 calls, 0.46 to 0.57 M tokens in, 0.12 to 0.19 M out,
  10 to 14 minutes per run (instead of about 2 hours locally), so about 70 minutes for the six runs.
  These runs are the new baseline: they are not comparable with the local model's.
* Spread of `linkding` (mean, min to max over 3 runs, `hidden` / `shown`): judged domain recall
  39% (33 to 50) / 56% (50 to 67); judged feature recall 65% (25 to 90) / 55% (15 to 85); judged feature
  precision 41% (5 to 67) / 27% (6 to 43); features generated 90 (67 to 110) / 77 (66 to 88). A change
  smaller than these ranges is noise. The model finds 4 to 7 domains and often puts technical areas
  (tests, build scripts, deployment) in their own domains.
* Audit of the judge's pairs (one run per series, `linkding`, read by a model against
  `reference.yaml`, not by a person): 23 of 68 feature pairs wrong (34%, 20 of 51 in `hidden`, 3 of 17 in
  `shown`) and all 4 domain pairs wrong. Frequent errors: REST API features paired with feature or asset
  features of the web interface, tag features paired with saving a bookmark, the bookmarklet paired
  with the browser extensions, interface mechanics paired with a business capability. 57 checked pairs
  were added to `benchmark/linkding/matches.yaml`.
* Business language, hand reading of 40 narratives of one run (half scored below 1 by the deterministic
  measure, half at 1): 29 business and 11 technical (queues, migrations, collations, Docker and
  coverage scripts, live reload). The deterministic score is below 1 for 8 of the 11 technical ones, but
  76% of the narratives are at exactly 1, so its mean (95%) saturates and tells little. The judge, run
  again on the sample, called 19 technical: it counted business objects (an API token, a Bookmark),
  JSON and an API client as code terms. After telling it in its prompt that those are business
  language, it agrees with the reading on 36 of 40 (28 of 40 before) and the 4 left are narratives it
  finds business and the reader technical. Weighted by the share of each score, about 79% of that run's
  narratives are in business language, close to the judge's 81% for the full run.

# Approach

1. Run `linkding` three times per series in a terminal without the 2 h limit of background commands
   (`nohup just benchmark benchmark/linkding/reference.yaml --runs 3 > ...`, about 12 hours), then audit
   a sample of the judge's pairs as the skill describes.
2. Read a sample of narratives by hand (rated business and rated technical by each measure) and decide
   which measure to keep, or how to combine them; fix the other one or drop it.
3. Have a person read both `matches.yaml` files against `reference.yaml` and correct the pairs.
4. Cut the cost of a `linkding` benchmark, alone or together:
   * stop `generate` after the features pass (a `--until features` option, and the matching option of
     `benchmark.rs`) for the runs that only measure domains and features, about 30 minutes per series;
   * copy `repo-map.json` from one clone to the next (file summaries are cached by content hash), which
     saves 133 calls per run but removes that part of the variance;
   * pass `--no-confidence` to `generate` from the script (about 19 minutes and 170 k tokens in per
     run), the benchmark only shows the mean confidence;
   * run on a hosted model: `benchmark.rs` forces `OPENROUTER_API_KEY=unused`, so it must forward the
     real key when set, then use `--base-url` and `--model`. The baselines come from the local model:
     redo `delivery_router` first, which is cheap, to check the model follows the JSON formats, and
     keep one model for all the runs of a comparison.
5. Consider a `--series` option of `benchmark.rs` to run only one series, and `--judge-model` to judge
   with another model than the one that generated the docs.

# Resources

* `.claude/skills/quality-benchmark/SKILL.md` (run and audit procedure)
* `issues/done/quality_benchmark.md` (decisions and baselines)
* `benchmark/delivery_router/`, `benchmark/linkding/`
* ADRs 0019 to 0023

# Hints

* A change smaller than the range between runs is noise: never conclude from one run.
* Only public repositories go into the benchmark; the clones and logs stay in `/tmp`.
