# Roadmap

The planned work of the project, grouped into milestones, in the order they are meant to land: the user orders them.
A milestone becomes issues once it is shaped (`skill:create-issue`); the `priority` and `depends_on` of each issue file order them within a milestone (`cardinal run next-issue`).
States: `idea`, `shaped`, `in issues`, `done`; `cardinal run release` adds `- Released: v<version>` to a `done` one
and moves it to [ROADMAP-done.md](ROADMAP-done.md). A milestone carries no date unless the project gives it one.

The released milestones are in [ROADMAP-done.md](ROADMAP-done.md); the ones up to "A large application, one bounded
context at a time" are the phases 1 to 11 of the former `PLAN.md`, in their order. The dated findings behind them are
in `docs/findings.md` and the closed issues in `issues/done/`.

## The readers of the docs out of the pipeline

- Goal: every reader of the generated docs (`render`, `report`, the MCP server, the web interface, `ask`, the
  benchmark) loads them in one place, through a versioned format and types that do not belong to the pipeline, in a
  crate of their own; `retrodoc-mcp` is left as the MCP server.
- Why: the project review of 2026-11-07 found three loaders of the same docs that diverge on the edits, readers
  depending on the whole pipeline for a few YAML files, and no format version on the saved artifacts; any export,
  hosted or third-party reader would start from there.
- State: in issues
- Issues:
  - [one_loader_of_the_generated_docs](issues/one_loader_of_the_generated_docs.md)
  - [format_version_of_the_saved_artifacts](issues/format_version_of_the_saved_artifacts.md)
  - [reader_types_in_core](issues/reader_types_in_core.md)
  - [docs_crate_out_of_the_mcp_crate](issues/docs_crate_out_of_the_mcp_crate.md)

## A local model, measured

- Goal: one complete benchmark run of an application on the local model, with its figures and duration next to the
  hosted baseline, or the principle of `PRODUCT.md` that a small local model is a valid target amended.
- Why: the only complete local attempt on linkding (2026-10-27) failed on the 120 s timeout; the loopback default is
  900 s since, and no local run has been made again.
- State: in issues
- Issues:
  - [complete_local_benchmark_run](issues/complete_local_benchmark_run.md)

## Read by someone taking an application over

- Goal: the generated docs of an application read by a person who takes it over (developer, product owner), with
  what they found right, wrong, missing and useless, written as dated findings.
- Why: the users of `PRODUCT.md` are assumed: only the author has read generated docs so far, and the benchmark
  measures likeness to a reference, not use.
- State: in issues
- Issues:
  - [read_by_someone_who_knows_the_application](issues/read_by_someone_who_knows_the_application.md)
