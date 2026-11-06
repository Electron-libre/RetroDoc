# retrodoc-agent

The agent of `retrodoc ask` ([ADR 0047](../adr/0047-the-ask-agent-is-a-crate-over-the-read-only-tools.md),
`specs/ask.md`). `ask` sends the question with a system prompt and offers the seven read-only tools of
`retrodoc_mcp::catalog` (`read_tools`, never a writing one). Each tool call is answered by `Docs::call_read_tool`,
the call the MCP server makes, run off the async threads; a call to an unknown tool or with arguments the tool
refuses gets the error as its result, so the model can correct it. The calls of a round go back in the history with the reasoning of the answer that made them, if the model wrote one (a thinking model, TCL-19). `ask_with` takes the tools offered:
`Tools::DocsOnly` (the `ask_replay` measure of `retrodoc-bench`) leaves out `read_source` and `git_log`, the sentence
of the system prompt that names them, and answers a call to a tool not offered with the names of the offered ones (ASK-21). The first answer without tool calls is the
answer. The tool results of a question are bounded so they fit the context of a local model: the calls of a round past
`MAX_CALLS_PER_ROUND` (6) and a result that would bring the results past `MAX_RESULT_CHARS` (40000 characters,
errors left out) are answered with an error telling the model to narrow down or answer (ASK-23, ASK-24).
After `max_rounds` rounds of calls (`DEFAULT_MAX_ROUNDS`, 8) one last request offers no tool and asks for the
answer from what was read; it sends the system prompt and one user message, the question then each call that got
no error with its result as text, so no server refuses a tool history without tools nor two user messages in a row
(ASK-22). `Answer` holds its text, the tool results of the question, the rounds and its sources:
the names the text gives in code spans (a run of backticks closed by a run of as many, a fenced block skipped) that
`Docs::source_named` resolves (a domain, a feature, a use case, a cited file, by its full id or path) and whose full id
a tool result of the question showed; an error, a "Not documented" or a "Not available" result shows nothing, and a
search does not show the query it repeats as its title, so a name the model gives without having read it is left
out. Its warnings come from the read model, not from the text:
`Docs::warnings_of` gives each feature and use case among the sources below the low-confidence threshold, then each
file among them or cited by one of them that changed since the docs were generated (`Freshness`). `Answer::render`
shows the text (`NO_ANSWER` when it is blank), then the sources (or `NO_SOURCE` when there are none), then the
warnings.

`Conversation` asks questions one after the other (`specs/chat.md`, for `retrodoc chat` and, one per browser session,
the ask page of `retrodoc ui --ask`, `specs/web-ask.md`): the request of a question
sends, before it, each earlier question as a user message and its answer text as an assistant message, never their
tool calls nor results (CHT-2, CHT-3); the last request without tools folds them as text before the question (CHT-7).
The turns kept fit in `MAX_MEMORY_CHARS` (10000 characters), the oldest dropped first (CHT-4), and their size is taken
off the budget of the tool results of the question (CHT-5). The sources of the turns kept count as shown, so a follow-up
naming what an earlier answer rested on keeps it as a source (CHT-6); a failed question is not kept (CHT-8). `ask` is
the same loop with no earlier turn. `Conversation::ask_observed` tells an `Observer` each tool call of the loop as a
`Step` (the tool, and the `id`, `query` or `path` of its arguments), before its result goes back to the model, so the
ask page can show it as it runs (ASK-25); `Conversation::ask` is it with an observer that does nothing.

Tested in `src/tests.rs` with the scripted `FakeLlm` of `retrodoc-pipeline` (feature `testing`), and through the binary
in `crates/retrodoc-cli/tests/ask.rs` and `chat.rs`, whose fake HTTP server answers tool calls or an HTTP error
(`Turn`) and keeps the messages of each request (`Call::messages`).
