# 0034. Domains from the surface by default

Status: Accepted (`files` kept as a value: superseded by 0035)

## Context

ADR 0020 shipped the `surface` path (domains and features built from the entities and the entry points)
behind `[pipeline] domains_from = "surface"`, the default staying `files` (LLM summaries of every file and
directory, ADR 0004) until the `surface` features were understood on a small library and a second large
application agreed. ADR 0029 added a third value, `contexts`, the `surface` passes run a bounded context at a
time.

The measures of `issues/done/surface_domains_default_decision.md` (deepseek-chat, 2026-10-28):

* On Lobsters (a Rails application, 373 source files), one `surface` run against one `files` run, same scoring:
  judged domain recall 100% against 43% (precision 82% against 38%), feature recall 97% against 100% (precision
  47% against 51%), use cases fully in business language 82% against 74%, 126 calls against 351, 9 minutes
  against 18. The repo map alone is 185 calls and 180k prompt tokens there.
* On linkding (2026-10-10, ADR 0020): 55% fewer calls, about 45% less time, judged feature recall at par.
* On `delivery_router` (a 9-file library), 5 runs per path: features at par once two runs that the judge left
  unpaired are audited by hand (about 95% recall and 94% precision against 88% and 88%); one run of five still
  makes features of constructors and internal collections, and the `surface` domains are 2 to 5 where `files`
  makes one.

## Decision

`domains_from` defaults to `surface`, and `init` writes it. `files` stays a value a config can name, until
`issues/done/single_pipeline_path.md` decides what of it to keep. `contexts` stays an opt-in for large applications:
it needs the map of `retrodoc contexts` first and documents a slice of the application per run, which a
default `generate` must not require.

A config that names `domains_from = "files"`, as every `init` wrote from 2026-10-09 to this decision, keeps
the `files` path: changing a value the user's file states would change their run without notice. New configs
and configs without the key take the new default, those `init` wrote before the key existed included. A
`surface` run that finds no entity and no entry point warns and names `domains_from = "files"`.

Rejected: keeping `files` as the default. It costs a call per file and directory before any business
reading, and its domains were the weakest figure on both applications; the small library, where it was ahead,
is at par after the audit.

## Consequences

* A first run of a new repository makes about a third of the calls on an application, and no repo map is
  written by default.
* The small library keeps a known flaw on the `surface` path: entry points that are constructors or internal
  collections become features in some runs (ADR 0008 lists a library's public API as its entry points).
* Every pass downstream is now tested and measured first on the `surface` path; the `files` path keeps its
  tests until it is removed or kept by `issues/done/single_pipeline_path.md`.
* Supersedes the "default stays `files`" statement of ADR 0020's status section; the rest of ADR 0020 stands.
