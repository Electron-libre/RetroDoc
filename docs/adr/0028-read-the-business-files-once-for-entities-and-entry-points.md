# 0028. Read the business files once for the entities and the entry points

Status: Accepted

## Context

The glossary and the entry points passes read the same business files (the files `business_files` locates)
with two prompts and two series of calls (ADR 0020 step 6). On the benchmark web application every file
the glossary reads is also read by the entry points pass: 36% of the input of the two passes
(`examples/read_overlap.rs`).

## Decision

`business_read` reads the business files once: one call per batch answers the entry points (with their
outputs) and the entities, over chunks of 5,000 characters. The answers are written to `glossary.yaml` and
`entry-points.yaml` with the per-file hash the two passes already compute, so they run right after, find
those files up to date and read only the rest (the entry point files outside the business list, and any
file the joint answer missed). The artifacts, the hashes and everything downstream are unchanged. It needs
one model for both passes (ADR 0023); otherwise, or without business files, each pass reads on its own. The
standalone `glossary` and `entry-points` commands are unchanged; `retrodoc business-read` runs the joint
reading alone. `generate` runs it by default; `[pipeline] business_read = false` turns it off.

The prompt lists the entry points first, in the task and in the answer shape: with the entities first, a
small library gave one entry point instead of about fifteen.

## Consequences

* On the benchmark web application (local model, three runs per variant): 18 calls instead of 28 (-36%),
  54.0k tokens in instead of 80.7k (-33%), about 20% less time, the entry points found within the noise of
  the separate reading (groups of the reference 93, 93, 79% against 86, 86, 79%).
* On a nine-file Ruby library it is worse: 62% of the groups in three runs against 50 to 100% separate,
  with fewer entries. That is why it can be switched off, and why one dedicated prompt per pass stays the
  reference for the files outside the business list.
* The answer type has room for the business rules of ADR 0021.
* The model sometimes copies the `--- path ---` header as the file name: the attribution now ignores the
  dashes, which also helps the separate passes.
