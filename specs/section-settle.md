---
id: STL
status: accepted
adrs: [0047, 0048, 0049, 0057]
---

# Settle a disputed section with the agent

## Intent

From the form of a section it doubts, the reader of `retrodoc ui --ask` asks the agent to help settle it: the agent
checks the section against the code, the history and the project's docs, asks the questions that would settle it,
and ends on a proposed review and correction that the reader puts in the form and saves, or not (ADR 0057).

## Requirements

### Behaviour

- **STL-1** (deterministic) With `--ask`, the review of a domain, a feature or a use case (`#review`, SFM-1) must hold,
  under its form, a button "Settle with the agent" that starts a settling of its section; without `--ask`, it holds
  none.
- **STL-2** (deterministic) Starting a settling must empty the exchanges and the conversation of the ask session, as
  WAK-22 does, send the settling message of the section (STL-4) as its first question, and show the page `/ask`
  following it (WAK-30).
- **STL-3** (deterministic) The page `/ask` of a session whose conversation a settling started must show, above its
  exchanges, the section being settled with a link to its page, until the session is reset (WAK-22) or another
  settling starts.
- **STL-4** (deterministic) The settling message must hold the kind and the id of the section, the answer of its
  detail tool (`get_domain`, `get_feature` or `get_use_case`: its text as shown, its review and its edit), and each
  contradiction of the section with the project's docs (REC-5: the doc section, what it states, what the section
  states), or say that the project's docs contradict none of it.
- **STL-5** (deterministic) Each question of a settling conversation, its first and the answers of the reader that
  follow it, must be sent with the settling system prompt, which asks the model to check each claim of the section
  against the code, the history and the docs with the tools, to say what it could not settle, to ask the reader the
  questions that would settle it, and to propose its verdict with `propose_verdict` once settled.
- **STL-6** (deterministic) A question of a settling conversation must offer the tool `propose_verdict` besides the
  read-only tools; a question of a conversation no settling started, and `retrodoc ask` and `chat`, offer none.
- **STL-7** (deterministic) An exchange whose answer holds a proposal must show it under its text: the status, the
  comment and each field of the text it gives, and a link "Fill the form" to the form of the section (SFM-6) with
  `?proposal=<n>`, `<n>` the number of the question in its session.
- **STL-8** (deterministic) The page of the settled section asked with `?proposal=<n>` and the cookie of the session
  that holds the exchange `<n>` must show its form filled with the proposal, the fields of the text it gives
  unfolded and the others as the page shows them, and a notice that the agent proposed it and nothing is saved
  until the form is.
- **STL-9** (deterministic) A settling must write neither `.retrodoc/reviews.json` nor `.retrodoc/edits.json`, from
  its start to its proposal shown in the form: only a save of the form writes them (SFM-3).

### Contract

- **STL-10** (deterministic) A settling must start by a `POST` to `/ask/settle` with the fields `kind` (`domain`,
  `feature` or `use_case`) and `id`, its session found or started as a question's (WAK-18), answered before the
  model answers: with the header `HX-Request`, with the header `HX-Redirect` naming `/ask`, else with a redirect
  `303` to `/ask`.
- **STL-11** (deterministic) A start of a settling must be refused, the session left as it was and no LLM call made,
  with the status 404 for a section the last run does not have, 400 for another kind, and 409 while a question of
  the session runs (WAK-29); without `--ask`, `/ask/settle` is answered as `/ask` is (WAK-2).
- **STL-12** (deterministic) The button of STL-1 must ask a confirmation (its `data-confirm`) saying that the
  conversation of the ask page starts over, only when the session of the page holds an exchange or a question
  running.
- **STL-13** (deterministic) The tool `propose_verdict` must take the arguments of the table below; a call whose
  arguments the table refuses gets the error as its result, the question going on (ASK-4).

  | argument | required | value |
  |---|---|---|
  | `status` | yes | `validated`, `wrong` or `to_review` (REV-2) |
  | `comment` | yes | the reason of the verdict, its evidence (files, commits, the reader's answers) |
  | `name`, `description` | no | the corrected text of the field |
  | `narrative` | no | the corrected narrative; refused for a domain or a feature (SED-3) |

- **STL-14** (deterministic) A call of `propose_verdict` that the table accepts must end the question with that
  proposal, the calls after it in its round not run; the text of the answer is the text of the message that made
  the call, or the comment of the proposal when that text is blank.
- **STL-15** (deterministic) The answer kept in the conversation for the next questions (CHT-2) of an exchange with a
  proposal must hold the proposal as text, its status, comment and fields, so that the reader can have it amended.
- **STL-16** (deterministic) A page of a section asked with `?proposal=<n>` that names no session kept in memory,
  no exchange `<n>` of the session, an exchange with no proposal or the proposal of another section must be shown as
  without it, with no notice.

## Scenarios

- **STL-H1** (happy; STL-1, STL-2, STL-4, STL-5, STL-7, STL-8, STL-9) Given a use case reviewed `wrong` with the
  comment "the clerk does not pay", when the reader presses "Settle with the agent", then the ask page follows a
  first question whose message holds the use case, its review and its contradictions; the agent reads its code and
  asks who pays; the reader answers "the customer"; the agent proposes `wrong` with a corrected description; the
  reader follows "Fill the form", finds the status, the comment and the description filled, saves, and only then
  are the review and the edit written.
- **STL-H2** (happy; STL-4, STL-6, STL-14) Given a feature that a section of the project's docs contradicts, when its
  settling starts, then the first question names the doc section and both statements, and the agent, having read
  the code, proposes `validated` with a comment citing the file, without a question to the reader.
- (STL-11) Given a question running in the session, when a settling starts, then it is answered with 409 and the
  running question and the exchanges are kept.
- (STL-11) Given a feature id the last run does not have, when a settling starts, then it is answered with 404, and
  no LLM call is made.
- (STL-13) Given a domain, when the agent proposes a narrative, then the call gets an error and the question goes on.
- (STL-14) Given a round of a call of `propose_verdict` then a call of `read_source`, then the question ends with the
  proposal and `read_source` is not run.
- (STL-15) Given an exchange that proposed `wrong`, when the reader answers "keep it to review", then the request of
  that question holds the proposed status and comment in the earlier answer.
- (STL-16) Given the exchange 2 proposing for the feature `orders/list-orders`, when the page of `orders/pay` is
  asked with `?proposal=2`, then its form is as without it.
- (STL-12) Given a session with no exchange, then the button asks no confirmation; given one with an exchange, it
  asks one.
- (STL-3) Given a settled session, when the reader resets it, then the page no longer names the section.

## Non-goals

- No transcript of the exchange kept with the review: the proposed comment carries the evidence the reader keeps.
- No buttons of choices for the questions of the agent: it asks in its text and the reader answers in the question
  field, as a follow-up of `chat`.
- No write by the agent: `propose_verdict` records a proposal of the exchange, nothing on disk (ADR 0057).
- No settling from the terminal (`retrodoc chat`) nor of a business rule, a glossary term or an actor.
- No proposal from the last request without tools (ASK-6): a settling that ran out of rounds ends on a text answer.
- No proposal kept across a restart of `ui`: it lives with its exchange in the session (WAK-21).
