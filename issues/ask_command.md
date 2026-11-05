---
priority: high
depends_on: [tool_calling_in_llm]
why: 'the command the milestone is for: a user without an external agent can question the docs'
---
# Answer a question about the application with `retrodoc ask`

# Goal
The MCP server serves an external agent; a user without one has no way to question the generated docs. Expected:
`retrodoc ask "<question>"` answers from the generated docs, the collected docs and, when needed, the code and the
git history, citing the features, use cases and files it relied on, warning when an answer rests on a low
confidence or on docs older than the code, and saying "not documented" rather than guessing (steps 3 and 4 of
`docs/findings.md` §4, without the interactive chat).

# Findings
* 2026-11-05, read from the code: the read-only tools exist as methods of `retrodoc_mcp::Docs` (`list_domains`,
  `get_domain`, `get_feature`, `get_use_case`, `search_docs`, `read_source`, `git_log`), with confidence, freshness,
  reviews and edits already in their answers (`specs/mcp-server.md`, `specs/docs-search.md`).
* The search answers "not documented" to 17 of 21 undocumented topics and keeps 19 of 21 documented ones
  (`issues/done/search_relevance_threshold.md`); the two lost use a word the docs don't have.

# Approach
1. A new crate `retrodoc-agent` (tool loop, prompts), depending on `retrodoc-llm`, `retrodoc-mcp` and
   `retrodoc-core`: the model navigates with the tools of `Docs`, in a bounded number of rounds (a local model has a
   small context).
2. The answer: its text, then the sources it cited (checked against the tool results, not trusted from the
   model), then the warnings the tools gave (confidence below the threshold, stale files).
3. `retrodoc ask` in `retrodoc-cli`, with the LLM settings of a new pass (`[llm.passes.ask]`), its tokens counted;
   read-only, no write to the target repository.
4. Tests with a fake provider scripting the tool calls, like the pipeline passes.

# Specs
* A new spec `specs/ask.md`, written at `issue-workflow` step 1.

# Milestone
Ask the documentation

# Resources
* `docs/findings.md` §4 (design, requirements, risks), `docs/crates/render-mcp-cli.md`, `crates/retrodoc-mcp/src/tools/`,
  `specs/mcp-server.md`, `specs/docs-search.md`, ADR 0016.
* The new crate changes the architecture diagram of `CLAUDE.md` and `docs/ARCHITECTURE.md` §2, and
  `commit_scopes` in `.claude/cardinal.toml`.

# Hints
* Over the size limit of one deliverable: the crate and its loop, then the command, then the warnings.
* The misses ("not documented") could feed the debt report: out of scope here, an issue of its own later.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
`retrodoc ask "<question>"`: a bounded tool loop over the seven read-only tools of `Docs`, then the answer, its
sources (code-span names a tool result held) and its warnings (low confidence, stale files) computed from the read
model. Out of scope: `chat`, misses fed to the debt report, answer quality, thinking models (own issues).

## Plan
1. [x] Crate `retrodoc-agent` and its loop: tools, dispatch to `Docs`, bounded rounds with a last request without
   tools, spec and ADR 0047 drafts; verified by unit tests with `FakeLlm::scripted`; makes true ASK-1 to ASK-6. (autonomous)
2. [ ] Sources checked against the tool results: names in code spans of the text held by a tool result, with their
   kind (a lookup added to `Docs`), "no source" note; verified by unit tests; makes true ASK-7 to ASK-9.
3. [ ] `retrodoc ask` in `retrodoc-cli`: pass `ask` in `PASS_NAMES`, `[llm.passes.ask]`, `--max-rounds`, tokens in the
   recap, read-only, error before a first `generate`; architecture docs, commit scope `agent`; verified by binary
   tests with the fake HTTP server answering tool calls; makes true ASK-12 to ASK-16.
4. [ ] Warnings: low confidence and stale files of the sources, from the read model; verified by unit tests and one
   binary test; makes true ASK-10, ASK-11; spec `accepted`.
5. [ ] Docs of the whole issue and fixes of its `high` review.

## Decisions
- Mode: autonomous
- Base commit: 9952baba1920d568c6bef03eec907c5681f6f98f
- The usage history is written in `.retrodoc/` like the other commands that call the LLM; nothing else (1a).
- At the round limit, one last request without tools asks for the answer (2a).
- 8 rounds by default, `--max-rounds` changes it (3a).
- An answer with no verified source is shown with a note that no source of the docs backs it (4a).

## Deliverable 1 summary
- done: crate `retrodoc-agent` (`ask`: bounded tool loop, last request without tools) over
  `retrodoc_mcp::catalog` (the seven read tools as data and `Docs::call_read_tool`), which the MCP server now uses
  for its descriptions, schemas and defaults; ADR 0047 accepted; architecture docs, `docs/crates/agent.md`, commit
  scope `agent` (moved here from deliverable 3: the crate lands here).
- checks: tests ok, clippy ok, specs ok (ASK draft), docs updated.
- red: ASK-6: the tests were written with the code; with the last-round message removed, the test fails
  (`assertion left == right failed`, `crates/retrodoc-agent/src/tests.rs:155`).
- Code review: no findings
- not verified: no real model call yet (the command comes in deliverable 3).
