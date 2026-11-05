---
id: TCL
status: accepted
adrs: [0046]
---

# Tool calls of an LLM provider

## Intent

A request to an LLM provider can offer tools, the model can answer with calls to them instead of text, and the next
request gives their results back, so an agent (the `ask` of the milestone "Ask the documentation") can navigate the
generated docs by itself. The calls go through `OpenRouter` and `DeepSeek` (and a local OpenAI-compatible server) in
the native OpenAI-compatible format, and each round is counted like any other call.

## Requirements

- **TCL-1** (deterministic) A request that offers tools must send each of them as a `tools` entry of type `function`
  with its name, its description and the JSON schema of its arguments as `parameters`.
- **TCL-2** (deterministic) A request that offers no tool must not send a `tools` field.
- **TCL-3** (deterministic) An answer whose message holds `tool_calls` must give each call back to the caller with its
  id, its tool name and its arguments as the raw text the server sent, in the order of the answer.
- **TCL-4** (deterministic) An answer whose message has a `null` or missing `content` next to its tool calls must be
  read as an answer with an empty text, not as an invalid response.
- **TCL-5** (deterministic) An assistant message that carries tool calls must be sent with those calls in its
  `tool_calls`, each with its id, the type `function`, its name and its arguments.
- **TCL-6** (deterministic) A tool result message must be sent with the role `tool`, the id of the call it answers as
  `tool_call_id` and the result as its `content`.
- **TCL-7** (deterministic) A tool call whose arguments are not valid JSON must reach the caller unchanged, not fail
  the call.
- **TCL-8** (deterministic) The usage, heartbeat and language wrappers must pass the tools of a request to the
  provider they wrap and the tool calls of its answer back to their caller unchanged.
- **TCL-9** (deterministic) Each answered round of a tool loop must be counted by the usage tracker as one call with
  its tokens, like a call without tools.
- **TCL-10** (deterministic) The fake provider of the tests must answer a scripted sequence of tool calls and texts
  and record the messages of each request it received, tool calls and results included.
- **TCL-11** (deterministic) An answer whose message holds `"tool_calls": null` must be read as an answer without
  tool calls, not as an invalid response.
- **TCL-12** (deterministic) An answer whose message has a `null` or missing `content` and no tool call must fail the
  call as an invalid response.
- **TCL-13** (deterministic) A tool call whose arguments are `null` or missing must reach the caller with the
  arguments `{}`.
- **TCL-14** (deterministic) A tool call without an id must reach the caller with an id no other call of the answer
  has.
- **TCL-15** (deterministic) The provider must not send again without its temperature or its schema a request that
  offers tools and fails with a client error (400, 404 or 422) whose body says the tools or function calling are not
  supported and names neither the temperature nor the schema (`temperature`, `response_format`, `schema`, `json`,
  `structured`).
- **TCL-16** (deterministic) The error of a request refused as in TCL-15 must say that the model or its server
  refuses tools, and name the model and the server's answer.
- **TCL-17** (deterministic) An answer whose message holds a `reasoning_content` must give it to the caller with the
  text and the tool calls.
- **TCL-18** (deterministic) An assistant message that carries a reasoning must be sent with it as its
  `reasoning_content`; one without a reasoning must not send the field.
- **TCL-19** (deterministic) The agent of `ask` must give back, in the history of its next round, the reasoning of
  each answer that called tools.

## Scenarios

- (TCL-1, TCL-3) Given a request offering `list_domains`, when the server answers a call `call_1` to it with `{}`,
  then the caller gets one tool call `call_1`, `list_domains`, `{}`.
- (TCL-5, TCL-6) Given the history user question, assistant call `call_1`, tool result `- Billing`, when it is sent,
  then the body holds the assistant message with its `tool_calls` and a `tool` message with `tool_call_id: call_1`.
- (TCL-4) Given an answer `{"content": null, "tool_calls": [...]}`, then the call succeeds with an empty text.
- (TCL-15, TCL-16) Given a request offering tools with a temperature set, when the server answers 400 `model does not
  support tools`, then one request only is sent and the error names the tools and the model.
- (TCL-15) Given a request offering tools, when the server answers 400 `unknown field temperature`, then the
  fallback without the temperature runs as for a request without tools; so does it for 400 `response_format is not
  supported together with tools` (the pair refused, ADR 0046) and for 400 `messages with role 'tool' must be a
  response to a preceding message with 'tool_calls'` (a history error, no refusal).
- (TCL-17, TCL-18) Given an answer with `reasoning_content: "look up the domains"` and a call, when the agent sends
  its next round, then the assistant message of that call holds `reasoning_content: "look up the domains"`.
- (TCL-9) Given a loop of three rounds, then the tracker counts three calls and the sum of their tokens.

## Non-goals

- The agent loop itself (how many rounds, which tools, the final answer): the issue of `retrodoc ask`.
- A JSON-action protocol over plain text for a server without native tools (ADR 0046).
- Streaming, parallel execution of the calls, or a `tool_choice` other than the server's default (`auto`).
- Checking the arguments against the tool's schema: the caller parses them.
