---
id: LTO
status: accepted
adrs: [0002, 0012]
---

# LLM request timeout

## Intent

Bound how long one LLM request may run, so that a hosted provider that hangs fails fast while a local
OpenAI-compatible server, much slower at writing a long structured answer, gets the time it needs without the
user having to know `llm.timeout_secs`.

## Requirements

- **LTO-1** (deterministic) With no `llm.timeout_secs`, a request whose `llm.base_url` names a loopback host
  (`localhost`, an address of `127.0.0.0/8`, `::1`) must time out after 900 seconds.
- **LTO-2** (deterministic) With no `llm.timeout_secs`, a request to the provider's own endpoint (no `llm.base_url`)
  or to a `llm.base_url` whose host is not loopback must time out after 120 seconds.
- **LTO-3** (deterministic) A `llm.timeout_secs` set in `[llm]` or in `[llm.passes.<name>]` must be the timeout of
  the requests it applies to, whatever the host of the endpoint.
- **LTO-4** (deterministic) A request that times out while the server answers must not be sent again, and its error
  must name `llm.timeout_secs`.

## Scenarios

- (LTO-1) Given `base_url = "http://127.0.0.1:11437/v1/chat/completions"` and no `timeout_secs`, when the provider is
  built, then its timeout is 900 seconds.
- (LTO-1) Given `base_url = "http://[::1]:11435/v1/chat/completions"`, then the timeout is 900 seconds.
- (LTO-2) Given `base_url = "https://localhost.example.com/v1"`, then the timeout is 120 seconds: only the host
  itself counts, not a name that starts like it.
- (LTO-2) Given a `base_url` that does not parse as a URL, then the timeout is 120 seconds.
- (LTO-3) Given a loopback `base_url` and `timeout_secs = 60`, then the timeout is 60 seconds.

## Non-goals

- Telling a local server on another machine of the network from a hosted one: such a server sets
  `llm.timeout_secs`.
- Sizing the timeout from the model or the expected answer length.
