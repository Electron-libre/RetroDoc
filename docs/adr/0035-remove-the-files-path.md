# 0035. Remove the `files` path of `generate`

Status: Accepted

## Context

ADR 0034 made `surface` the default of `[pipeline] domains_from` and kept `files` (ADR 0004: an LLM summary of
every file and directory, the domains clustered from them, every file placed in a domain, ADR 0017) as a value
a config can name, until `issues/single_pipeline_path.md` decided what of it to keep. Every change of a pass
downstream had to hold on both paths, and the `files` path alone is about 3,300 lines of `retrodoc-pipeline` and
`retrodoc-cli`: the repo map and its cache, the clustering with its coverage and repair, the features per
domain, the batching of small files (`llm.batch_chars`, ADR 0011).

The measures behind ADR 0034: on two applications the `surface` domains were well ahead (Lobsters: judged domain
recall 100% against 43%) for about a third of the calls; on a 9-file library the features are at par once
audited. Nothing outside the `files` path reads the repo map: MCP freshness reads `source-hashes.json`, the
report reads the scope and the behaviour coverage.

## Decision

`generate` has one path from the surface: `surface`, or `contexts` (ADR 0029) for a large application a bounded
context at a time. The `files` path is removed with what only it uses: the repo map, its summary cache, the
clustering by directory with its coverage guarantee and the placement of unassigned files, the features per
domain from the file summaries, `llm.batch_chars` and `[llm.passes.repo-map]`.

A `retrodoc.toml` that still names `domains_from = "files"`, `[llm] batch_chars` or `[llm.passes.repo-map]` is
refused with a message that names the removal and the line to change, as a removed key is (SET-4): silently
running another path would change the user's run without notice. A `surface` run that finds no entity and no
entry point still warns, and says the run documents no domain.

Rejected: (b) `surface` by default with `files` kept as a fallback for an empty surface: the fallback needs the
repo map, the clustering and the features per domain, which is nearly all of the code to remove, and every pass
would still be tested on both. (c) Both kept by kind of repository: the small library, the only case where
`files` was ahead, is at par after the audit (ADR 0034).

## Consequences

* One path to test and measure; about 3,300 lines less.
* A repository whose surface is empty (a language the reading passes don't read, no entry point) gets no domain
  and lists its files as technical support, where `files` documented every file.
* The "every file in a domain" coverage is gone; the behaviour coverage (ADR 0020) reports the unreached files
  instead.
* Sub-domains are only ever filled by the `files` path; their render, report and MCP code becomes dead and is a
  follow-up.
* Supersedes ADR 0004 and ADR 0017, and the batching part of ADR 0011; ends the "`files` stays a value" clause
  of ADR 0034.
