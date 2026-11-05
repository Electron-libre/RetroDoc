# 0047. The `ask` agent is a crate of its own over the read-only tools of the MCP server

Status: Accepted

## Context

`retrodoc ask` answers a question from the generated docs with an LLM navigating them (`docs/findings.md` §4). The
read-only tools exist as functions of `retrodoc_mcp::Docs`, answering in Markdown with confidence, freshness and the
files they rest on; `retrodoc-llm` offers tools and reads the calls (ADR 0046). The pipeline is deterministic and
writes the docs; the MCP server is LLM-free (ADR 0016). The answer must cite its sources and warn when they are
uncertain, and a model can name sources it never read.

## Decision

A new crate `retrodoc-agent` (tool loop, prompts) depends on `retrodoc-llm` and `retrodoc-mcp`; the
CLI calls it. Its tools are the seven read-only functions of `Docs` under the names of the MCP server, offered from one catalog
the server uses too (`retrodoc_mcp::catalog`), never the
writing ones. The loop is bounded (a local model has a small context): after `max_rounds` rounds of calls, one last
request without tools asks for the answer.

The sources and the warnings are not trusted from the model: the sources are the names its text gives in code spans
that a tool result of the same question held; the warnings (confidence below the threshold, stale files) are computed
from the read model for those sources, not parsed from the tool results nor from the text.

Rejected: the loop in `retrodoc-pipeline` (it writes the docs and caches its passes; the agent is read-only and
caches nothing) or in `retrodoc-mcp` (LLM-free by ADR 0016); a final `answer` tool carrying the sources as arguments
(one more thing a small model gets wrong, and the sources would still have to be checked).

## Consequences

The architecture diagram gains `retrodoc-cli ──> retrodoc-agent ──> retrodoc-mcp, retrodoc-llm`, and
the commit scopes `agent`. `retrodoc-mcp` exposes the kind, confidence and stale files of a name to the agent. The
same agent serves the `chat` of a later issue.
