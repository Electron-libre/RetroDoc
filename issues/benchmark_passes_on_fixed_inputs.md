# Benchmark each pass on fixed inputs, against references a human has checked

# Goal
Make a pipeline change measurable for a few cents and a few minutes. Today a comparison is whole
`generate` runs, three or more per setting, judged by an LLM: the passes upstream vary from run to run, so
the effect of a change in one pass is drowned, and the judge is wrong on a large share of its pairs. Replay
one pass on saved inputs, compare with a reference the user has read, and keep the whole-run benchmark for
the final check.

# Findings (project review of 2026-10-12)
* Only the entry points pass has a replay against a hand-made reference (`entry_points_replay`,
  `benchmark/<repo>/entry-points.yaml`); the confidence checks have an agreement measure
  (`confidence_agreement`). Domains, features, use cases and rules are measured through whole runs.
* The spread of whole runs is as large as most changes (`issues/done/quality_benchmark.md`); the features pass
  had to be replayed by hand on fixed domains to compare settings (`issues/brief_injection_experiments.md`).
* Audit of the judge: 10 of 23 feature pairs wrong on `delivery_router`, 3 of 22 on linkding; it overstates
  feature recall by 8 to 17 points and precision by 12 to 33.
* The references were drafted by the agent; "a human spot check of `matches.yaml` is still due"
  (`issues/done/quality_benchmark.md`). The two measures of business language disagree (judge about 54%,
  deterministic score 88 to 93%) and nobody settled it by reading.
* Two repositories: a 9-file library, outside the target, and one application of about 180 source files.
* The rules have no reference at all.

# Approach
1. Human review first: the user reads `reference.yaml`, `matches.yaml` and `entry-points.yaml` of both
   repositories and a sample of narratives (business language or not), and corrects them.
2. A frozen set of inputs per repository, saved from one good run (roles, business files, glossary, entry
   points, then domains, then features), kept outside the clone so a replay starts from it.
3. A replay per pass (domains on fixed surface, features on fixed domains, use cases and rules on fixed
   features): N runs of that pass alone, the figures of the reference for that level, calls and tokens.
4. A rules reference for linkding (the hand measure of `issues/done/business_rules.md` is a start).
5. Decide whether a second application joins `benchmark/`, and whether `delivery_router` stays a benchmark
   or only a smoke repository.

# Resources
* `crates/retrodoc-pipeline/src/benchmark/`, `examples/entry_points_replay.rs`,
  `examples/confidence_agreement.rs`
* Skill `quality-benchmark` (`benchmark.rs`), `benchmark/`
* `issues/done/quality_benchmark.md`, `issues/done/benchmark_spread_and_judge_bias.md`

# Hints
* The frozen inputs depend on the model that wrote them: say which, and refresh them when a pass upstream
  changes its artifact format.
* The steps that need the user's reading cannot be done by the agent alone.
* Do it before the issues that compare settings (`measure_model_split_per_pass`,
  `brief_injection_experiments`), which need this measure.

# Tracking
1. [x] Check the references with a subagent (`reference.yaml`, `matches.yaml`, `entry-points.yaml` of both
   repositories, a sample of narratives, the two measures of business language): report of gaps, factual
   fixes only. Checked by an agent, not by a human: the human spot check stays due.
2. [x] Frozen inputs per repository (loader, `~/retrodoc-runs/frozen/<repo>/`, manifest with model and commit).
3. [x] Domains replay on the frozen surface.
4. [x] Features replay on the frozen domains.
5. [ ] Use cases replay on the frozen features.
6. [ ] Rules reference for linkding and rules replay.
7. [ ] Docs (commands, pipeline, skill `quality-benchmark`) and the decision on a second application and on
   `delivery_router`.

## Decisions
* Replays are `examples/` in Rust with a shared loader of frozen inputs, no CLI subcommand.
* Frozen inputs live in `~/retrodoc-runs/frozen/<repo>/`.
* LLM of the replays: DeepSeek. N = 3 runs per setting. Announce the cost before each real run.
* The review of the references is done by a subagent, not by the user.
* Item 5 of the approach (second application, fate of `delivery_router`) is decided by the user once the
  figures exist.
* Subagent check of 2026-10-23 (no human): no clear factual error in `reference.yaml`, `entry-points.yaml`;
  no file changed. Left for the human spot check: actor `Dispatcher` (delivery_router, not an actor), 3
  delivery_router and about 15 linkding pairs of `matches.yaml` that are loose (API facets paired with
  "Use the REST API", multi-feature items), the feature "Use the browser extensions" (not verifiable in this
  repository), and entry-point regexes too broad in linkding (`bookmark`, `api|client|token|...`, `login|...`).
  Missing from the linkding reference: bundles, RSS feeds, tag merge/edit, favicons and previews, reader view,
  background tasks, operations commands, public endpoints. Narratives in business language: about 55% on
  linkding (20 read) and 54% on delivery_router (13 read), so the judge (about 54%) is right and the
  deterministic score (88 to 93%) only measures identifier leaks; the share of use cases at score 1 (61 to
  77%) is closer but still high.
* The linkding reference gained 4 features the subagent found missing (merge and rename tags, bundles, feed
  reader, reader view) and the actor "Feed reader", by agent: recall figures of linkding are not comparable
  with the benchmarks before 2026-10-23. Replays of use cases report the judge's business-language rate; the
  deterministic score is reported apart as "identifier leaks only".
* The frozen domains of the `files` path hold paths and no resources, so the features pass of the `surface`
  path finds no entry point in them: the frozen domains of both repositories were replaced by domains
  clustered from the surface by `deepseek-chat` (`pass_replay domains --freeze-first`, recorded in the
  manifest as `domains_origin`). The replays follow the `surface` path, not the default `files` one.
