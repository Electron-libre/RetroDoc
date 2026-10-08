# 0024. DeepSeek as a second provider, same wire format

Status: Accepted

## Context

ADR 0002 made OpenRouter the only provider. The quality benchmark (`issues/done/benchmark_spread_and_judge_bias.md`)
needs many runs of a hosted model: on the local model one `linkding` run takes about two hours, and
OpenRouter adds a layer between the user and the model they want. The DeepSeek API speaks the same
chat-completions format.

## Decision

`provider = "deepseek"` is accepted next to `"openrouter"`. Both use `OpenRouterProvider`; only the default
endpoint (`https://api.deepseek.com/chat/completions`) and the default key variable differ: when
`api_key_env` is left at its OpenRouter default, `DEEPSEEK_API_KEY` is read. `llm.base_url` still overrides the
endpoint. The CLI reads a git-ignored `.env` at startup (variables already set win) so keys need not be
exported. No provider-specific adapter: a provider with another wire format still needs its own ADR.

## Consequences

A benchmark run of `linkding` takes about 10 to 14 minutes and about $0.30 on `deepseek-chat`, against two hours
locally. The trait stays the boundary, so another OpenAI-compatible provider only needs a new match arm in
`OpenRouterProvider::from_config`. The type keeps its name although it now serves two providers.
