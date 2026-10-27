# Crate: retrodoc-llm

Detail moved out of `CLAUDE.md`. The LLM boundary: `docs/ARCHITECTURE.md` §8.

- **retrodoc-llm** (`types.rs`, `heartbeat.rs`, `openrouter.rs`, `usage.rs`): `LlmProvider` trait abstraction (kept provider-agnostic) + `OpenRouterProvider`, which serves
  `provider = "openrouter"` and `"deepseek"` (same chat-completions format, own endpoint and key variable; ADR 0024), a real `reqwest` HTTP client with exponential-backoff retry on
  429/5xx that honors the delay the server asks for (`Retry-After`, Google's `retryDelay`; up to 120 s, a longer
  one is an error). Per-request HTTP timeout is 120 s, 900 s for a loopback `llm.base_url` (`localhost`, `127.0.0.0/8`, `::1`: a local
  model writing a long JSON answer needs it, LTO-1 to LTO-3), unless `llm.timeout_secs` is set (a timeout is not retried, the same prompt would take as long again: the error says so and names `llm.timeout_secs`). `CompletionRequest.json_schema` is the optional shape of a JSON answer, sent as a strict `response_format` (plus OpenRouter's `require_parameters` on its real endpoint) unless `llm.structured_output = false`; a 400/404/422 is sent again without it, and if that works the schema is left out for the rest of the run (ADR 0023). `llm.temperature` (also per pass) is sent as `temperature` when set, and unset it is 0 for the passes of `ZERO_TEMPERATURE_PASSES` (every pass but `contexts`, `repo-map` and the benchmark judge, ADR 0031), outside the fingerprints; a 400/404/422 is sent again without it (schema kept), and if that works it is left out for the rest of the run. `CompletionResponse.usage`
  is the optional token count the server reports (`usage.rs`: a missing or malformed block is `None`, never estimated; `cached_tokens` is the part of the prompt served from the server's cache, read from `prompt_tokens_details.cached_tokens` or DeepSeek's `prompt_cache_hit_tokens`, 0 when absent or malformed and capped at the prompt, TOK-1 to TOK-3);
  `LlmProvider::model()` is the model a provider asks for when a request names none (empty by default): passes put it in their fingerprints. `UsageProvider` + `UsageTracker` count answered calls and tokens per pass and per model (concurrency-safe; the CLI names
  the pass with `set_pass`, or `in_pass`, which closes it when the work ends). Failed attempts and internal retries are not counted.
