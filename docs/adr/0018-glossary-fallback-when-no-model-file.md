# 0018. Read the logic and entrypoint files when no model file gives an entity

Status: Superseded by 0025

## Context

ADR 0008 builds the glossary from the files the roles pass classes as `model`. That classification is an
LLM judgment made from the file tree alone, and it varies from run to run. On a small Ruby library whose
domain classes are plain objects under `lib/`, ten runs of the roles pass on the same repository gave a
`model` rule that matched files in five, and none in the other five. With no model file the glossary is
empty, the surface carries no business concept and the domain clustering loses the hints it was designed
around. In three of the five empty runs the whole `lib/` tree was classed `entrypoint` (the public API of
a library), in the other two `logic`.

## Decision

When the `model` files give no entity, `build_glossary` reads the `logic` files, then the `entrypoint`
ones, at most 30 files in path order, with a dedicated prompt that keeps only the classes holding
business data and rules (no services, collections or helpers). The entities go in the same
`glossary.yaml` with the same per-file content hash, so a rerun reads nothing again. `generate` also logs
an `info` line when source files exist and the surface still has no entity.

Rejected: fixing it in the roles prompt alone (the answer stays random, kept as a complement, not as the
only protection), and a `retrodoc.toml` key for the limit (a safety net, not a setting).

## Consequences

The surface is no longer empty on a repository without a `models/` folder, at the price of one extra LLM
call in that case. The 30 files are chosen by path, not by relevance, so on a large repository without
models the fallback sees a sample; ranking them is a possible follow-up. `logic` and `entrypoint` files
that gave no entity are remembered as empty, so a repo whose classes are all technical pays the call once.
