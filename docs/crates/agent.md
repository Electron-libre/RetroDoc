# retrodoc-agent

The agent of `retrodoc ask` ([ADR 0047](../adr/0047-the-ask-agent-is-a-crate-over-the-read-only-tools.md),
`specs/ask.md`). `ask` sends the question with a system prompt and offers the seven read-only tools of
`retrodoc_mcp::catalog` (`read_tools`, never a writing one). Each tool call is answered by `Docs::call_read_tool`,
the call the MCP server makes, run off the async threads; a call to an unknown tool or with arguments the tool
refuses gets the error as its result, so the model can correct it. The first answer without tool calls is the
answer. After `max_rounds` rounds of calls (`DEFAULT_MAX_ROUNDS`, 8) one last request offers no tool and asks for the
answer from what was read. `Answer` holds its text, the tool results of the question, the rounds and its sources:
the names the text gives in code spans that `Docs::source_named` resolves (a domain, a feature, a use case, a cited
file, by its full id or path) and that a tool result of the question held, the name or its full id; a name the model
gives without having read it is left out. Its warnings come from the read model, not from the text:
`Docs::warnings_of` gives each feature and use case among the sources below the low-confidence threshold, then each
file among them or cited by one of them that changed since the docs were generated (`Freshness`). `Answer::render`
shows the text, then the sources (or `NO_SOURCE` when there are none), then the warnings.

Tested in `src/tests.rs` with the scripted `FakeLlm` of `retrodoc-pipeline` (feature `testing`).
