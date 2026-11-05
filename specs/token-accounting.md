---
id: TOK
status: accepted
adrs: [0015]
---

# Token accounting

## Intent

Tell, per pass and per model, what a run cost in tokens, including the prompt tokens a server served from its
prompt cache, so that a change of prompt order meant to reuse that cache can be measured; and set it against the
estimate printed before the run (`specs/run-estimate.md`), so that an estimate that drifts is seen.

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
- **TOK-6** (deterministic) A `generate` that printed its estimate and succeeded must save it with its run in the
  usage history: the source files and changed files it rested on and, per pass, its basis and the low and high bounds
  of its calls, prompt tokens and completion tokens; an older history without it, or with one it can't read, must
  still load, its runs with no estimate.
- **TOK-7** (deterministic) The recap of a `generate` that succeeded must show, per pass of its estimate that was
  estimated or made a call, whether the measured calls, prompt tokens and completion tokens fell within their range
  and, for each that did not, the bound it missed and the miss in percent of that bound, a token count below its range
  said to be a lower bound when some calls of the pass reported no usage; a failed run or one with no estimate must
  keep the recap of today.
- **TOK-8** (deterministic) The measuring tool of the ratio table must report, per pass, over the measured runs whose
  history holds an estimate, how many fell within the range of their calls, prompt tokens and completion tokens, how
  many above and below it with the median miss of each side, leaving out a token count below its range when some
  calls reported no usage; it must say so when no measured run holds an estimate.

## Scenarios

- (TOK-1) Given a response with `prompt_tokens` 1000 and `prompt_cache_hit_tokens` 800, when the usage is read, then
  it is 1000 prompt tokens of which 800 cached.
- (TOK-2) Given a response with `cached_tokens` set to `"many"`, when the usage is read, then the usage keeps its
  prompt and completion counts and no cached tokens.
- (TOK-3) Given a server that reports a cached count above its prompt count, when the call is recorded, then the
  cached count is capped at the prompt count.
- (TOK-4) Given a run whose servers reported no cached count, when the recap is printed, then it is the recap of today.
- (TOK-6) Given a `usage.json` written before the estimate existed, when the history is loaded, then every run loads
  with no estimate.
- (TOK-7) Given `use-cases` estimated at 100-300 calls that made 360, when the recap is printed, then its line says
  the calls are 20% above the high bound.
- (TOK-7) Given `confidence` estimated at zero because it was off, and a run that made no call to it, when the recap
  is printed, then the pass gets no line; had it made calls, it would get one, as a miss above zero.
- (TOK-7) Given a `generate` that fails in its `domains` pass, when the recap is printed, then it sets no pass
  against the estimate, and the run is saved with no estimate.
- (TOK-7) Given a miss above a high bound of zero, when the recap is printed, then it shows the bound and the
  measured figure, without a percent.

## Non-goals

- Pricing: a cached token is not billed here at a lower rate.
- Estimating the cache a server did not report.
- Controlling or requesting a cache (no provider-specific cache directive is sent).
- Correcting the embedded ratio table from the misses: the table is rebuilt by hand from the measured runs.
- Comparing the estimate of a command other than `generate`: only `generate` prints one.
