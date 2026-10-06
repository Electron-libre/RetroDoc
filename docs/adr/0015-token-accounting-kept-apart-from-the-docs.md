# 0015. Token accounting: what the server reports, kept apart from the generated docs

Status: Accepted

## Context

Nothing counted LLM calls or tokens (0011 listed it as open), and the per-pass timings of the first full run
on a real repo were lost with its terminal log. Servers return a `usage` block (`prompt_tokens`,
`completion_tokens`), but not all of them, and not always well-formed. Passes run one after another, with
concurrency only inside a pass (0011). The rendered `_retrodoc/run-metadata.json` must not change between two
runs on unchanged input (0010), and figures that depend on the run would change it every time.

## Decision

- Count what the server reports and nothing else. `CompletionResponse.usage` is optional; a missing,
  incomplete or malformed block is `None` and never fails the call. Calls without it are counted apart and
  the recap says the token sums are then a lower bound. No tokens are estimated from characters.
- A wrapper provider (`UsageProvider`) feeds a shared `UsageTracker`; the CLI names the current pass with
  `set_pass`. This relies on passes being sequential, so a call is attributed to the pass open when it ends.
  Only answered calls are counted: failed attempts and the retries inside `OpenRouterProvider` report no
  tokens, so a flaky server makes the figures a lower bound.
- Every command that calls the LLM ends with a recap, and the last 20 runs are saved in
  `.retrodoc/cache/usage.json` with the command name. It lives with the caches, not in the docs dir, so the
  rendered docs and the no-op rerun are untouched, and `generate --force` keeps the history.
- Tokens only, no prices. A price key in `retrodoc.toml` was considered and dropped: one price pair cannot
  be right when several models are used, and a per-model table would duplicate provider price lists that
  change. The amount is left to the reader.

## Consequences

Pass timings and token counts survive the terminal log and can be compared run to run; the saved history is
the input a pre-run estimate for the whole pipeline needs (PLAN §7.2, item 7, step 2). Figures are a lower
bound against an unstable server, and the ingestion time before the first pass is not counted. A second
provider must also return `usage` for its tokens to show.
