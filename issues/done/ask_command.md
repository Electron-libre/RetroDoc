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
2. [x] Sources checked against the tool results: names in code spans of the text held by a tool result, with their
   kind (a lookup added to `Docs`), "no source" note; verified by unit tests; makes true ASK-7 to ASK-9. (autonomous)
3. [x] `retrodoc ask` in `retrodoc-cli`: pass `ask` in `PASS_NAMES`, `[llm.passes.ask]`, `--max-rounds`, tokens in the
   recap, read-only, error before a first `generate`; architecture docs, commit scope `agent`; verified by binary
   tests with the fake HTTP server answering tool calls; makes true ASK-12 to ASK-16. (autonomous)
4. [x] Warnings: low confidence and stale files of the sources, from the read model; verified by unit tests and one
   binary test; makes true ASK-10, ASK-11; spec `accepted`. (autonomous)
5. [x] Docs of the whole issue and fixes of its `high` review. (autonomous)

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

## Deliverable 2 summary
- done: `Docs::source_named` (`retrodoc-mcp`, `tools/sources.rs`) resolves a name to a domain, feature, use case or
  cited file by its full id; the agent keeps as sources the code spans of the answer that resolve and that a tool
  result held; `Answer::render` shows the text then the sources, or the no-source note.
- checks: tests ok, clippy ok, specs ok (ASK draft), docs updated (`docs/crates/agent.md`, `render-mcp-cli.md`).
- red: ASK-7, ASK-8, ASK-9: `left: [] right: [Source { kind: Feature, id: "billing/pay-invoice" }, …]` and an empty
  render, before the code.
- Code review: no findings
- not verified: nothing left out.

## Deliverable 3 summary
- done: `retrodoc ask "<question>" [--max-rounds N]` (`commands/ask.rs`): docs loaded first (fails before any call
  without a `generate`), provider of the pass `ask` (`PASS_NAMES`, `[llm.passes.ask]`, kept out of `[output]
  language`: a reading pass), calls counted under `ask`, prints `Answer::render`; the fake HTTP server of the binary
  tests answers tool calls (`Turn`, `FakeLlm::turns`); `main` sets the logs in `init_logs` (over 100 lines
  otherwise). Docs: README, CLAUDE.md, ARCHITECTURE §3, `render-mcp-cli.md`.
- checks: tests ok, clippy ok, specs ok (ASK draft, ASK-10 and ASK-11 left), docs updated.
- red: ASK-6, ASK-12, ASK-13, ASK-14, ASK-15, ASK-16: `error: unrecognized subcommand 'ask'`.
- Code review: no findings
- not verified: no real model call (a paid or local run is for the user to ask).

## Deliverable 4 summary
- done: `Warning` and `Docs::warnings_of` (`retrodoc-mcp`, `tools/sources.rs`): low confidence of the features and
  use cases among the sources, then the files among them or cited by them changed since the generation, each once;
  the agent sets them on `Answer` (off the async threads) and `render` prints them after the sources. Spec `ask`
  accepted.
- checks: tests ok, clippy ok, specs ok (accepted, all traced), docs updated (README, ARCHITECTURE §3,
  `docs/crates/agent.md`, `render-mcp-cli.md`).
- red: ASK-10, ASK-11, ASK-12: `left: None right: Some(LowConfidence { … confidence: 0.3 })`, `left: [] right:
  [Stale { path: "app/mailer.rb", deleted: false }]`, and no `Warnings:` in the output of `retrodoc ask`.
- Code review: no findings
- not verified: no real model call.

## Deliverable 5 summary
- done: docs of the whole issue (README crate count, `docs/findings.md` §4 step 3, `retrodoc-mcp` crate doc,
  `docs/crates/agent.md`); fixes of the `high` review, spec amended (ASK-7 reworded, ASK-17 to ASK-20 added).
- `high` review: 10 findings.
  1. A failed call echoing a name made it a source: fixed, a source needs its full id in a result that showed
     something (ASK-17).
  2. No budget of the tool results per round or in all: issue `issues/ask_loop_always_answers.md`.
  3. The last request drops the tools with a tool history some servers refuse: same issue (not checked on a real call).
  4. Code spans read by parity of backticks: fixed, runs of equal length (ASK-7 scenario).
  5. A blank last answer printed as an empty answer: fixed, `NO_ANSWER` (ASK-18).
  6. schemars schemas with `["integer","null"]` and `format`: fixed in the catalog (ASK-19).
  7. Arguments `null` refused: fixed, read as `{}` (ASK-20).
  8. `cited_paths` rebuilt per lookup: not fixed, an answer holds a few dozen spans and the cost is nothing next to a
     call to the LLM.
  9. Stale lookup duplicated: fixed, `Docs::stale_states` shared by the tools and the warnings.
  10. Unknown-tool list hardcoded: fixed, drawn from `read_tools`; the server keeps its typed `rmcp` handlers, which
     share the descriptions, schemas' structs and defaults of the catalog.
- checks: tests ok, clippy ok, specs ok (accepted, all traced), docs updated.
- red: ASK-7, ASK-17, ASK-18, ASK-20: `left: ["b", "then:", "ls", …] right: ["a ` b", "`", "billing", "app/x.rb"]`,
  `left: [Source { kind: Feature, id: "billing/pay-invoice" }, …] right: []`, a blank render, `Error: invalid arguments
  for list_domains: not a JSON object`; ASK-19: written with its fix (the catalog test widened).
- Code review: no findings
- not verified: no real model call (`issues/ask_question_set.md` measures the answers).
