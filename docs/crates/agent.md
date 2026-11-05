# retrodoc-agent

The agent of `retrodoc ask` ([ADR 0047](../adr/0047-the-ask-agent-is-a-crate-over-the-read-only-tools.md),
`specs/ask.md`). `ask` sends the question with a system prompt and offers the seven read-only tools of
`retrodoc_mcp::catalog` (`read_tools`, never a writing one). Each tool call is answered by `Docs::call_read_tool`,
the call the MCP server makes, run off the async threads; a call to an unknown tool or with arguments the tool
refuses gets the error as its result, so the model can correct it. The first answer without tool calls is the
answer. After `max_rounds` rounds of calls (`DEFAULT_MAX_ROUNDS`, 8) one last request offers no tool and asks for the
answer from what was read. `Answer` holds its text, the tool results of the question and the rounds.

Tested in `src/tests.rs` with the scripted `FakeLlm` of `retrodoc-pipeline` (feature `testing`).
