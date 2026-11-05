---
id: CHT
status: accepted
adrs: [0047]
---

# Question the docs in a conversation

## Intent

`retrodoc chat` lets a user question the generated docs in a conversation: each question is answered like
`retrodoc ask` (`specs/ask.md`: tools, sources, warnings, "not documented"), and the earlier questions and answers
are kept, so a follow-up does not restate its context, within a memory bounded for the context of a local model.

## Requirements

- **CHT-1** (deterministic) `retrodoc chat` must read one question per line of its input, answer each, print each
  answer like `retrodoc ask` (text, sources, warnings), and end at the end of the input or on a line `exit` or
  `quit`; a blank line is no question.
- **CHT-2** (deterministic) The request of a question must send, after the system prompt and before the question,
  each earlier question kept in memory as a user message followed by its answer as an assistant message, oldest
  first.
- **CHT-3** (deterministic) The memory must keep an earlier turn as its question and the text of its answer only:
  its tool calls and tool results are not sent again.
- **CHT-4** (deterministic) The memory must drop its oldest turns until its questions and answers fit in
  `MAX_MEMORY_CHARS` (10000 characters), the turn just answered included (dropped too when it alone is longer).
- **CHT-5** (deterministic) The budget of the tool results of a question (ASK-24) must be `MAX_RESULT_CHARS` minus
  the characters of the memory sent with it.
- **CHT-6** (deterministic) The sources of an answer must also count as shown the sources of the earlier turns kept
  in memory, so a follow-up naming what an earlier answer rested on keeps it as a source.
- **CHT-7** (deterministic) The last request of a question (ASK-22) must keep one user message: the earlier turns
  kept, as text, then what ASK-22 sends.
- **CHT-8** (deterministic) A question whose LLM call fails must print the error and let the next question be
  asked; the failed turn is not kept in memory.
- **CHT-9** (deterministic) `retrodoc chat` must call the model with the settings of the pass `ask`, count its
  tokens under the pass `ask` in the recap printed at its end, write no file of the target repository other than its
  usage history in `.retrodoc/`, and fail before any LLM call on a repository with no generated docs.

## Scenarios

- (CHT-2, CHT-3) Given a first question answered after a call of `get_feature`, when a second question is asked,
  then its first request holds the system prompt, the first question, its answer, the second question, and no tool
  result.
- (CHT-4) Given three turns of 4000 characters each, when the third is answered, then the memory holds the second
  and the third only.
- (CHT-6) Given a first answer whose source is `orders/list-orders`, when the second answer names
  `` `orders/list-orders` `` without a tool call, then it is a source of the second answer.
- (CHT-8) Given a server that fails the first question, then the error is printed and the second question is
  answered.

## Non-goals

- A summary of the dropped turns by the model: they are dropped, not condensed.
- Saving a conversation or resuming it in a later `chat`.
- A line editor (history, arrow keys): the input is read line by line, so a scripted input works like a terminal.
- A pass `chat` of its own in `[llm.passes]`: `chat` is `ask` in a conversation.
