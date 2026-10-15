# Crate: retrodoc-llm

Detail moved out of `CLAUDE.md`. The LLM boundary: `docs/ARCHITECTURE.md` §8.

- **retrodoc-llm** (`types.rs`, `heartbeat.rs`, `openrouter.rs`, `usage.rs`): `LlmProvider` trait abstraction (kept provider-agnostic) + `OpenRouterProvider`, which serves
  `provider = "openrouter"` and `"deepseek"` (same chat-completions format, own endpoint and key variable; ADR 0024), a real `reqwest` HTTP client with exponential-backoff retry on
  429/5xx that honors the delay the server asks for (`Retry-After`, Google's `retryDelay`; up to 120 s, a longer
  one is an error). Per-request HTTP timeout is 120 s unless `llm.timeout_secs` is set (a slow local
  model writing a long JSON answer needs more; a timeout restarts the whole generation on retry). `CompletionRequest.json_schema` is the optional shape of a JSON answer, sent as a strict `response_format` (plus OpenRouter's `require_parameters` on its real endpoint) unless `llm.structured_output = false`; a 400/404/422 is sent again without it, and if that works the schema is left out for the rest of the run (ADR 0023). `llm.temperature` (also per pass) is sent as `temperature` when set, and unset it is 0 for the extraction passes (`EXTRACTION_PASSES`) and nothing for the writing ones (ADR 0030), outside the fingerprints; a 400/404/422 is sent again without it (schema kept), and if that works it is left out for the rest of the run. `CompletionResponse.usage`
  is the optional token count the server reports (`usage.rs`: a missing or malformed block is `None`, never estimated);
  `LlmProvider::model()` is the model a provider asks for when a request names none (empty by default): passes put it in their fingerprints. `UsageProvider` + `UsageTracker` count answered calls and tokens per pass and per model (concurrency-safe; the CLI names
  the pass with `set_pass`, or `in_pass`, which closes it when the work ends). Failed attempts and internal retries are not counted.
