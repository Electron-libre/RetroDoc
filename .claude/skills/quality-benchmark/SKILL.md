---
name: quality-benchmark
description: Measure the quality of the generated functional docs against a hand-written reference on a public repository (recall and precision of domains and features, business-language narratives, cost), with the repo's own docs hidden then shown, and compare with the previous benchmark. Use when asked to benchmark quality, to know whether a pipeline change makes the docs better, or before and after a step of the redesign (ADRs 0019 to 0023).
---

# Quality benchmark

`skill:smoke-test` checks that `generate` is robust; this checks that what it writes is right. The
reference is written by hand per repository in `benchmark/<repo>/reference.yaml` (purpose, actors,
domains, features, a sample of use cases and rules, the pinned commit) and the hand-written pairs in
`benchmark/<repo>/matches.yaml`. Only **public** repositories go in the benchmark.

## Run

```sh
just benchmark benchmark/<repo>/reference.yaml                 # 1 run per series
just benchmark benchmark/<repo>/reference.yaml --runs 3        # the spread needs 3 at least
just benchmark <reference> --model <m> --base-url <u>          # another model or server
```

It takes minutes to hours per run (2 series × N runs), so launch it with `run_in_background` and read
the logs in `/tmp/retrodoc-benchmark-<repo>/<timestamp>/<series>/run-<n>.log`. Same local LLM rules as
the smoke test (Ollama on `:11435`, large context, don't run two at once).

What `benchmark.rs run` does: clones the repository of the reference at its pinned commit (cached in
`/tmp/retrodoc-benchmark-<repo>/source`); for each series, `hidden` (the repo's own docs left out of
`ingest.existing_docs_paths`) then `shown`, and each run, clones afresh, runs `retrodoc init`,
`generate` and `retrodoc benchmark --judge --out run-<n>.json`; then `retrodoc benchmark-table` writes
`table.md` with the mean, range and number of runs of each figure and its change against the previous
benchmark of the same repository.

## Reading the table

- **recall / precision** (no suffix): hand-written pairs and equal names only: the strict reading.
- **(judged)**: with the LLM judge's proposals added. The judge shares the biases of the model that
  generated the docs: read its pairs in `<clone>/.retrodoc/benchmark/judge.yaml`, correct them and copy
  the right ones to `matches.yaml`; they are then in the strict figures too.
- A change smaller than the range between runs is noise, not progress.

## After the run

- Report the table, the number of runs and what moved beyond the spread.
- A `FAIL` line (generate or benchmark failed in a run) is a finding: read the log around the first
  `WARN`/error. A run that failed is missing from the table, which says so in its `Runs` column.
- Don't commit the clones, logs or tables (they live in `/tmp`); commit reference and match files only.
