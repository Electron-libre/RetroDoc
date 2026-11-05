---
id: ASK
status: accepted
adrs: [0016, 0046, 0047]
---

# Ask a question about the application

## Intent

`retrodoc ask "<question>"` lets a user without an external agent question the generated docs: a model navigates
them with the read-only tools of the MCP server, and the command prints its answer, the sources it rests on and the
warnings those sources carry (low confidence, code changed since the docs), so the answer can be checked.

## Requirements

- **ASK-1** (deterministic) The agent must offer the model the tools `list_domains`, `get_domain`, `get_feature`,
  `get_use_case`, `search_docs`, `read_source` and `git_log`, each with a description and the JSON schema of its
  arguments.
- **ASK-2** (deterministic) The agent must not offer a tool that writes (a section edit, a review).
- **ASK-3** (deterministic) Each tool call must be answered, as its tool result, with the answer of the read-only
  tool of the MCP server of the same name for the same arguments.
- **ASK-4** (deterministic) A tool call must be answered with a tool result naming the error, not fail the question,
  when its tool is unknown or its arguments are not a JSON object the tool accepts.
- **ASK-5** (deterministic) The text of the answer must be the text of the first model answer that holds no tool call.
- **ASK-6** (deterministic) The agent must make one last request offering no tool, asking the model to answer from
  what it read, after `max_rounds` answers holding tool calls (8 unless `--max-rounds` sets it).
- **ASK-7** (deterministic) The sources of an answer must be the domains, features, use cases and files its text
  names in a code span whose full id or path a tool result of the same question held, each with its kind.
- **ASK-8** (deterministic) A name of the text that no tool result of the question held must not be listed as a
  source.
- **ASK-9** (deterministic) An answer with no source must say that no source of the docs backs it.
- **ASK-10** (deterministic) The answer must warn of each feature or use case among its sources whose confidence is
  below the low-confidence threshold, with that confidence.
- **ASK-11** (deterministic) The answer must warn of each file changed or deleted since the docs were generated that
  is among its sources or that a feature or use case among its sources cites.
- **ASK-12** (deterministic) `retrodoc ask` must print the text of the answer, then its sources, then its warnings.
- **ASK-13** (deterministic) `retrodoc ask` must call the model with the settings of `[llm]` changed by those of
  `[llm.passes.ask]`.
- **ASK-14** (deterministic) `retrodoc ask` must count the tokens of its calls under the pass `ask` in the recap it
  prints.
- **ASK-15** (deterministic) `retrodoc ask` must not write any file of the target repository other than its usage
  history in `.retrodoc/`.
- **ASK-16** (deterministic) `retrodoc ask` on a repository with no generated docs must fail with a message asking
  for `retrodoc generate`, before any LLM call.
- **ASK-17** (deterministic) A name that only an error, a "Not documented" or a "Not available" tool result, or the
  query a search repeats, held must not be listed as a source.
- **ASK-18** (deterministic) An answer whose text is blank must say that the model gave no answer.
- **ASK-19** (deterministic) The JSON schema of the arguments of a tool must not give a property a list of types nor
  a `format`, which some servers refuse.
- **ASK-20** (deterministic) A tool call whose arguments are `null` must be answered as a call with `{}`.
- **ASK-21** (deterministic) The agent asked without the code tools must offer neither `read_source` nor `git_log`,
  and its system prompt must not name them.

## Scenarios

- (ASK-4) Given the model calls `get_feature` with `{"slug": "x"}`, then the tool result names the missing `id` and
  the next request goes on.
- (ASK-6) Given a model that keeps calling tools and `--max-rounds 2`, when two answers held calls, then the third
  request offers no tool and its text is the answer.
- (ASK-7, ASK-8) Given the text names `` `billing/pay-invoice` `` that `get_feature` returned and
  `` `billing/refund` `` that no tool returned, then only `billing/pay-invoice` is a source, as a feature.
- (ASK-17) Given a call `get_feature` for `orders/list-orders` answered "Not documented: no feature
  `orders/list-orders`", when the text names `` `orders/list-orders` ``, then it is no source.
- (ASK-7) Given a text holding a fenced block of three backticks or a span of two before `` `billing` ``, then a
  span closes only on a run of as many backticks and `billing` stays a candidate source.
- (ASK-11) Given the source `billing/pay-invoice/pay-by-card` whose step cites `app/invoice.rb`, modified since the
  last `generate`, then the answer warns that `app/invoice.rb` changed.

## Non-goals

- The interactive `chat` and its memory (`issues/chat_command.md`).
- Feeding the questions answered "not documented" to the debt report.
- The quality of the answers, measured on a question set (`issues/done/ask_question_set.md`).
- A thinking model or a server that refuses tools (`issues/done/tool_loop_on_thinking_and_refusing_servers.md`).
- Retrieval by embeddings: the lexical search of `specs/docs-search.md` only.
