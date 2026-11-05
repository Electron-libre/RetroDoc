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
