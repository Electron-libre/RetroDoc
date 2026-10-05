# 0012. LLM request resilience: timeout, token cap, server-requested delays, heartbeat

Status: Accepted

_Retroactive ADR, reconstructed from the history (71b8e98, 9d0cc86, a81ee56, 79342b0, a4d55ff; 2026-09-28 to 2026-10-03)._

## Context

Free-tier OpenRouter models fail intermittently, and local models can generate for minutes or run away
on long structured answers. A stuck run and a slow call look the same on the terminal.

## Decision

In `OpenRouterProvider`:

- Retry with exponential backoff on 429 and 5xx, but when the server states a delay (`Retry-After`
  header, Google's `retryDelay` in the body) wait exactly that. Up to 120 s; a longer delay is an
  unrecoverable quota error, not a pause.
- Per-request HTTP timeout of 120 s, overridable with `llm.timeout_secs` for slow local models. A timeout
  restarts the whole generation on retry.
- `max_tokens` is always sent (8,192) to cap runaway generations; an answer cut by `finish_reason=length`
  logs a warning, and one that overflows twice is split (0006).

`HeartbeatProvider` wraps any provider in the CLI and logs "still waiting for the LLM (Ns)" every 30 s;
`progress.rs` prints one line per unit with rank, percentage, elapsed time and ETA.

## Consequences

Transient failures are absorbed and quota exhaustion fails fast with a clear error. The 8,192 cap bounds
answer size and drove the chunking and batching limits (0013). The timeout is a trade-off: too short
loops on slow models, hence the option.
