---
id: TOK
status: accepted
adrs: [0015]
---

# Token accounting

## Intent

Tell, per pass and per model, what a run cost in tokens, including the prompt tokens a server served from its
prompt cache, so that a change of prompt order meant to reuse that cache can be measured.

## Requirements

- **TOK-1** (deterministic) The usage of a completion must carry the number of prompt tokens the server reports as
  served from its cache, read from `prompt_tokens_details.cached_tokens` (OpenAI-compatible, OpenRouter) or from
  `prompt_cache_hit_tokens` (DeepSeek).
- **TOK-2** (deterministic) A server that reports no cached count must give a usage with zero cached tokens, never an
  estimate; a malformed cached count must not cost the answer nor the prompt and completion counts.
- **TOK-3** (deterministic) The totals of a pass, of a model and of a run must sum the cached tokens, and the cached
  tokens must never exceed the prompt tokens in any total.
- **TOK-4** (deterministic) The recap must show the cached prompt tokens next to the prompt tokens of the run and of
  each pass, and must show nothing about the cache when no call reported any.
- **TOK-5** (deterministic) The usage history file must keep the cached tokens, and an older history without them must
  still load, with zero.

## Scenarios

- (TOK-1) Given a response with `prompt_tokens` 1000 and `prompt_cache_hit_tokens` 800, when the usage is read, then
  it is 1000 prompt tokens of which 800 cached.
- (TOK-2) Given a response with `cached_tokens` set to `"many"`, when the usage is read, then the usage keeps its
  prompt and completion counts and no cached tokens.
- (TOK-3) Given a server that reports a cached count above its prompt count, when the call is recorded, then the
  cached count is capped at the prompt count.
- (TOK-4) Given a run whose servers reported no cached count, when the recap is printed, then it is the recap of today.

## Non-goals

- Pricing: a cached token is not billed here at a lower rate.
- Estimating the cache a server did not report.
- Controlling or requesting a cache (no provider-specific cache directive is sent).
