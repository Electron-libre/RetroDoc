---
priority: high
why: 'the estimate of the milestone extrapolates from these ratios; without them it is a guess off by 2-3x'
---
# Measure the calls and tokens of each pass per unit of its input

# Goal
A pre-run estimate (`pre_run_estimate`) needs, for each LLM pass of `generate`, how many calls and tokens (prompt,
cached, completion) one unit of its input costs: an entry point, a file read, a context, a domain, a feature, a use
case. The only ratios written down (`docs/findings.md` §3: ~0.24 features and ~0.56 use cases per file) predate the
top-down redesign and the passes it added. Expected: a table of measured ratios per pass, with their spread across
applications, kept in the repository as the data the estimate reads, from the runs already saved, with no new LLM run.

# Findings
* 2026-11-05: the directory of saved runs outside git (named in `CLAUDE.md`) holds 134 `generate` runs in the `.retrodoc/cache/usage.json` of the saved runs
  (`RunUsage`: per pass, per model, calls, calls without usage, prompt, cached and completion tokens, wall time,
  unparseable answers, skipped units). It records no count of the units a pass read: the ratios need the artifacts
  of the same run (entry points, contexts, features, use cases in `.retrodoc/cache/`).
* 2026-11-05: the `generate` of the Lobsters benchmark baseline (`dafea18`, deepseek-chat) made 236 calls for ~1.47M
  prompt tokens; `use-cases` alone 93 calls and ~1.04M prompt tokens (~70%), `confidence` 76 calls and ~205k,
  `business-read` 33 calls and ~96k; `glossary` and `reconcile` recorded no call.
* 2026-11-05: many saved runs are reruns (a cached unit makes no call) or ran under another code: only first
  complete runs of a known commit measure a ratio.

# Approach
1. List the saved runs whose `usage.json` holds a complete first `generate`, with their commit, model and
   application (the ledger names them); drop reruns and runs of a removed pass (`repo map`).
2. For each pass, pick its unit (what its calls scale with) and count it from the artifacts of the same run.
3. Compute calls and tokens per unit, the median and the spread across the applications (a small Rust repo, a Django
   app, a Rails app, a large Rails app), and record which passes are fixed cost (one call: `brief`, `roles`, ...).
4. Keep the table where the estimate reads it (a constant table in the pipeline, or a file under `benchmark/`), with
   the runs it comes from; a measuring example in `retrodoc-bench` can rebuild it from a directory of runs.

# Milestone
Pre-run cost estimate

# Resources
* `crates/retrodoc-pipeline/src/usage_log.rs` (`RunUsage`), `specs/token-accounting.md`, `docs/findings.md` §3 point 7,
  the ledger of measures named in `CLAUDE.md` (outside git), ADR 0015 (tokens only, no prices), ADR 0042 (measuring tools in
  `retrodoc-bench`).

# Hints
* No paid run: the saved runs are the input. Name no private repository in the table; describe it generically.
* Calls without usage make the token sums a lower bound: count them apart.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
Per LLM pass of `generate`, the calls and tokens (prompt, cached, completion, calls without usage apart) per unit
of its input, and the yields the estimate extrapolates (entry points per file, features per domain, use cases per
feature), median and range across applications, from the saved runs only. Out of scope: the estimate itself.

## Plan
1. [x] `pass_ratios` in `retrodoc-bench` (module + example): reads a directory of runs, keeps the complete first
   `generate` of each (every pass from `sources` on, no `repo-map`), counts the units from the artifacts of the same
   `.retrodoc/cache/` (business files read and their characters, entry point files, domains, features, use cases,
   matched reconcile items), writes the ratio table (per run, then median and range per application); verified by
   tests on hand-built run directories (a rerun and a run with `repo-map` left out, calls without usage counted apart,
   fixed-cost passes marked) (autonomous).
2. [x] The table: run over the saved runs, kept as a data file with the runs it comes from (no private name), the
   finding dated in `docs/findings.md` §3 point 7 (the old per-file ratios superseded), `docs/crates/bench.md`,
   `docs/dev/commands.md`; the ledger line; verified by rebuilding it identical from the runs (autonomous).
3. [x] Fixes of the `high` review of the whole issue (autonomous).

## Decisions
- Mode: autonomous
- Base commit: 2c86324de655b54b0a86c1037b1baa3ba2b9972b
- The large private Rails app (its only runs: 2026-10-15/16, local model, older code, resumed over 3 `generate`):
  included as indicative, out of the median (user, 2026-11-05).
- The table lives in `crates/retrodoc-pipeline/data/pass-ratios.yaml`, embedded by the estimate (user, 2026-11-05).
- No saved run of a Rust application: the small application is delivery_router (Ruby).
- Units (to confirm at gather from the code of each pass): fixed cost `sources`, `brief`, `roles` (a second call for
  unmatched files), `business-files`, `actors`, `domains`; `business-read` per file read and per 1,000 characters;
  `entry-points` per entry point file; `features` per domain (the `uncategorized` one included); `use-cases` and
  `rules` per feature; `confidence` per use case; `reconcile` per item matched to a section.

## Deliverable 1
- done: `retrodoc_bench::pass_ratios` (`measure_run`, `ratio_table`) and the example `pass_ratios`.
- checks: tests 6/6 of `pass_ratios`, `cardinal run project-check` green, docs (`bench.md`, `commands.md`, crate root).
- red: none (no spec, a measure; the tests were written with the module, not before it).
- Code review: no findings (`low`).
- Units confirmed at gather: `business-read` reads the files of `glossary.yaml` `models`; `entry-points` the files of
  `entry-points.yaml` outside them; `features` makes one call per domain of `domains.yaml` (`uncategorized` included)
  plus the placements; a pass turned off (`rules = false`, the benchmark runs) is absent from the history and its
  runs are left out of that pass.

## Deliverable 2
- done: `crates/retrodoc-pipeline/data/pass-ratios.yaml` from 22 runs (manifest `pass-ratios-runs.yaml` next to the
  runs, outside git), the dated finding in `docs/findings.md` §3 point 7, `docs/crates/pipeline.md`, the ledger line.
- `RatioTable::to_yaml`: one line per run, spread and application row (1,295 lines in block YAML, over the size
  limit of a deliverable; 181 now, the same values read back, tested).
- checks: rebuilt twice, byte-identical; no private name in the table (`large private Rails application`);
  `cardinal run project-check` green.
- red: none (a data file).
- Code review: no findings (`low`).
- not verified: the large private Rails app runs at older code with a local model (indicative, out of the spread);
  `rules` measured on two applications only (the benchmark runs turn it off); no Rust application among the runs.

## Review of the whole issue (`high`)
Code review: 10 findings.
- Fixed: a glossary read apart from the business files, a command before the first `generate` or a `generate` after
  it that called the LLM, and fewer `generate` in the history than asked now refuse the run; the characters of a file
  are counted up to what the readings send (40,000) and read lossily; one indicative run marks its application; the
  files missing from a tree are in the table; the commit of the indicative run is quoted in the manifest; the dated
  finding gives the date and models of the indicative run.
- Not fixed: the artifacts are read untyped, on purpose: the indicative run's are of an older format the typed
  loaders may refuse, and the tests cover the shapes read.
- Fixes reviewed (`low`): Code review: no findings.
- Table rebuilt: the medians move by less than 3% (business-read 0.157 -> 0.158 calls per 1,000 characters).
