---
id: WAK
status: accepted
adrs: [0039, 0047, 0048, 0049]
---

# Ask the documentation from the web interface

## Intent

`retrodoc ui --ask` lets the reader of the web interface question the docs in a conversation like `retrodoc chat`
(`specs/chat.md`), each answer with its sources as links to the pages of the interface and its warnings, without
leaving the browser; without `--ask` the interface calls no LLM (ADR 0048).

## Requirements

- **WAK-1** (deterministic) Without `--ask`, `retrodoc ui` must build no LLM provider: it starts with no API key set.
- **WAK-2** (deterministic) Without `--ask`, `/ask` must be answered with the status 404 and the not-found page, a
  `POST` too.
- **WAK-3** (deterministic) With `--ask`, `retrodoc ui` on a repository with no generated docs must fail before it
  listens, with the message of `retrodoc ask` (ASK-16).
- **WAK-4** (deterministic) With `--ask`, `retrodoc ui` must fail before it listens, naming the pass `ask`, when the
  LLM provider of the pass `ask` cannot be built (a missing API key).
- **WAK-5** (deterministic) With `--ask`, the questions must call the model with the settings of `[llm]` changed by
  those of `[llm.passes.ask]`, as `retrodoc ask` does (ASK-13).
- **WAK-6** (deterministic) With `--ask`, the page `/ask` must hold a form, sent with `htmx`, that sends a question by
  a `POST` to `/ask`.
- **WAK-7** (deterministic) A question sent to `/ask` must be answered as `retrodoc chat` answers a line
  (CHT-2 to CHT-8), after the earlier questions of the same browser session.
- **WAK-8** (deterministic) The page `/ask` must show the exchanges of the session, oldest first, each with its
  question, then the text of its answer, then its sources, then its warnings.
- **WAK-9** (deterministic) The text of an answer must be rendered from its Markdown as a collected doc is, its raw
  HTML shown as text (WEB-49) and a link with a scheme other than `http`, `https` or `mailto` shown as its text alone
  (WEB-50).
- **WAK-10** (deterministic) Each source of an answer must be labelled with its kind and id, and be a link to its
  page: a domain to `/domains/<domain>`, a feature to `/domains/<domain>/<feature>`, a use case to
  `/domains/<domain>/<feature>/<use case>`, a file to `/code/<path>`.
- **WAK-11** (deterministic) A use case source whose id names no domain (its feature is not in the run) must be shown
  without a link.
- **WAK-12** (deterministic) An answer with no source must say that no source of the docs backs it (ASK-9), and one
  whose text is blank that the model gave no answer (ASK-18).
- **WAK-13** (deterministic) Each warning of an answer (ASK-10, ASK-11) must be shown under its sources.
- **WAK-14** (deterministic) A question `POST` with the header `HX-Request` must be answered at once, before the model
  answers, with the running exchange of WAK-30 alone, without the layout of the page, for the form to add it after
  the former ones; any other must be answered with a redirect `303` to `/ask` once the question is answered.
- **WAK-15** (deterministic) The running exchange must say that the docs are being read, and the button of the form
  must stay disabled from the sending of a question until its answer or its error is shown.
- **WAK-16** (deterministic) A question whose LLM call fails must end with an exchange that shows the error, served
  with the status 200 by WAK-31 and WAK-33, the question not kept in the conversation (CHT-8).
- **WAK-17** (deterministic) A blank question must be answered with the status 400, and no LLM call.
- **WAK-18** (deterministic) A question sent to `/ask` without the cookie of a session kept in memory must start a
  new session and set the cookie `retrodoc_ask` to its id, `HttpOnly`, `SameSite=Strict`, `Path=/`; the id must be 128
  random bits written in hexadecimal.
- **WAK-19** (deterministic) Two sessions must not share their exchanges nor their conversation.
- **WAK-20** (deterministic) At most 32 sessions must be kept in memory: a new one past them drops the session used
  least recently.
- **WAK-21** (deterministic) A session must keep its last 50 exchanges for the page; the conversation keeps its own
  memory (CHT-4).
- **WAK-22** (deterministic) The page `/ask` must hold a button that sends a `POST` to `/ask/reset`, which empties the
  exchanges and the conversation of the session and answers with a redirect `303` to `/ask`.
- **WAK-23** (deterministic) With `--ask`, `retrodoc ui` must count the tokens of its calls under the pass `ask`, and
  print and save the recap of the calls under the command `ui` when an interrupt (Ctrl-C) stops it.
- **WAK-24** (deterministic) With `--ask`, `retrodoc ui` must not write any file of the target repository other than
  those of WEB-12 and its usage history in `.retrodoc/`.
- **WAK-25** (deterministic) The page `/ask` and `/ask/reset` asked without the cookie of a session kept in memory
  must start no session: the page shows no exchange and sets no cookie.
- **WAK-26** (deterministic) While a question of a session runs, the page `/ask` of that session must be answered
  without waiting for the model.
- **WAK-27** (deterministic) A file source must link to `/code/<path>` with its path percent-encoded as WEB-23 encodes
  the path of a doc.
- **WAK-28** (deterministic) A second interrupt must stop `retrodoc ui` at once, a question in flight dropped, with
  the recap of WAK-23.
- **WAK-29** (deterministic) A question sent while another question of the same session runs must be answered with
  the status 409, and no LLM call.
- **WAK-30** (deterministic) The running exchange of a question must show the question and the list of its steps, and
  connect with the `htmx` SSE extension, served by the interface as `htmx` is (ADR 0039), to
  `/ask/questions/<n>/events`, `<n>` the number of the question in its session.
- **WAK-31** (deterministic) `/ask/questions/<n>/events` must answer `text/event-stream`: one event `step` per tool
  call of the question (ASK-25), in order and from the first one on each connection, its data an item of the list
  of steps naming the tool and its target; then one event `answer` whose data is the exchange as WAK-8 to WAK-13
  show it, or its error (WAK-16); then the stream must end.
- **WAK-32** (deterministic) The running exchange must add each `step` to its list and be replaced by the data of the
  event `answer`; an exchange that ended must list its steps under a closed disclosure.
- **WAK-33** (deterministic) When its stream fails, the running exchange must be replaced by the answer of
  `/ask/questions/<n>`, the exchange alone, served once the question has ended.
- **WAK-34** (deterministic) `/ask/questions/<n>` and its events must be answered with the status 404 when the cookie
  names no session kept in memory or one that holds no question `<n>`, running or among its exchanges.
- **WAK-35** (deterministic) A question sent must run to its end when no page follows it (a tab closed), its exchange
  kept in its session; the page `/ask` must show a question still running as its running exchange (WAK-30).
- **WAK-36** (deterministic) Each exchange that ended must show, under its answer or its error, the LLM calls of the
  question with their tokens in (and among them those served from the cache) and out, then the same sums for the
  session since it started or was reset (WAK-22); the calls whose server reported no usage must be counted apart,
  their tokens not estimated.
- **WAK-37** (deterministic) The calls and tokens of WAK-36 must be those the recap of WAK-23 counts for the same
  calls.

## Scenarios

- (WAK-7) Given a first question answered after a call of `get_feature`, when a second question is sent with the same
  cookie, then the first request of the second holds the first question and its answer, and no tool result.
- (WAK-10) Given an answer that names `` `orders/list-orders` `` after reading it, and `` `app/order.rb` ``, then
  the page links `/domains/orders/list-orders` and `/code/app/order.rb`.
- (WAK-9) Given an answer whose text holds `<script>alert(1)</script>`, then the page shows it as text.
- (WAK-16) Given a provider that fails, when a question is sent, then the exchange shows the error and the next
  question is sent without it in its messages.
- (WAK-18) Given a cookie naming a session no longer kept (a restart, dropped by WAK-20), when a question is sent,
  then a new session starts.
- (WAK-20) Given 33 sessions started one after the other, then the first is no longer kept and the last 32 are.
- (WAK-14) Given a browser without scripts, then its question is redirected to `/ask`, which shows the exchange.
- (WAK-31) Given a question that calls `get_feature` for `orders/list-orders`, then answers, when its stream is read
  after the question ended, then it holds the step `get_feature` `orders/list-orders`, then the answer, then ends.
- (WAK-34) Given the question 1 of a session, when `/ask/questions/1` is asked with the cookie of another session,
  then it is answered with 404.
- (WAK-36) Given a first question of 2 calls of 100 tokens in and 10 out each, then a second of 1 call, then the
  second exchange shows 1 call, 100 tokens in and 10 out for the question, and 3 calls, 300 and 30 for the session.
- (WAK-36) Given a question failing after one answered call, then its exchange shows the error and that call.

## Non-goals

- No answer text streamed word by word: the answer shows whole once the question has ended.
- No cost in money: the calls and tokens only, the price of a token depending on the provider and its plan.
- No way to stop a running question from the page: it ends within its rounds (ASK-6).
- The wall time of the pass `ask` in the recap of `ui --ask` is the time the server ran, not the time spent
  answering: the calls of concurrent sessions would close the pass of one another.
- No conversation saved on disk nor resumed after a restart of `ui`: the sessions live in memory.
- No authentication: the server binds to `127.0.0.1` by default (WEB-1); the session cookie only keeps two tabs or
  browsers apart.
- No link from the code spans of the text of an answer: its sources are listed and linked under it.
- No pass `ask` of its own for the interface: `ui --ask` uses the settings of `ask`.
