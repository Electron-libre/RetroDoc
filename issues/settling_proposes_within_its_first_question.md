---
priority: medium
why: 'every first question of the second settling measure ran out of rounds before proposing; the domain got no proposal at all'
---
# Make a settling reach its proposal within the rounds of its first question

# Goal
The agent that settles a disputed section (`specs/section-settle.md`) spends the rounds of its first question checking
the section and reaches the last request, which offers no tool, before it calls `propose_verdict`. The reader then
gets a correction in the text instead of a filled form, or has to ask a second question. A settling whose evidence is
in hand should end its first question on a proposal, or on the questions only the reader can answer.

# Findings
* 2026-11-25, default smoke repository (a copy of a saved run), `deepseek-chat`, `ui --ask` with the default 8
  rounds, three settlings by the project's author (`docs/findings.md`, section 4, item 5, second measure).
* Each first question used its 8 rounds: 19 to 20 tool calls, 9 LLM calls, 57k to 64k tokens in, no proposal.
* The domain ended there, on a correction written in the text that the reader typed into the form by hand.
* The use case and the feature proposed at their second question, in 1 and 3 rounds.
* Cost of the three settlings: about $0.04 (31 calls, 199k tokens in, 146k of them cached, 10.6k out).

# Approach
1. Measure first: read the tool calls of a first question (the steps of the exchange, or a replay with the fake
   provider) to see what the rounds go to: rereading the same files, the history, the docs.
2. Candidate fixes, to combine:
   1. Offer `propose_verdict` in the last request of a settling question too (STL-5 and the non-goal on the last
      request change).
   2. Tell the model, in the settling prompt or in the result of a round near the end, how many rounds are left and
      to propose or ask before they run out.
   3. A budget of its own for a settling question (`--max-rounds` of the settling), with its cost measured.
3. Measure again on the same copy and sections (the ledger's planned measure), the usage read from the `ui` log.

# Specs
* `specs/section-settle.md`: amends STL-5 (the last request) and its non-goal "No proposal from the last request
  without tools".

# Milestone
Read by someone taking an application over

# Resources
* `specs/section-settle.md`, ADR 0057, `crates/retrodoc-agent/src/settle.rs`, `crates/retrodoc-agent/src/lib.rs`
  (the rounds of a question).
* `docs/findings.md`, section 4, item 5; the usage lines of the measure in the measures ledger.

# Hints
* A paid measure needs the user's go and the measures ledger read before.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-25

## Understanding
A settling question must reach `propose_verdict` (or its questions) within its rounds: the last request offers the
tool, the prompt states the rounds budget, and the `ui` log names the tool calls so the measure sees what the rounds
go to (the steps of the earlier measures were never captured). Out of scope: a budget of rounds of its own (measured
first). The measure again needs the user's go (cents on DeepSeek).

## Plan
1. [x] Log the tool calls of a settling question: one line per question after its usage line; verified by a unit
   test of the line and the settling page test; makes true STL-19. (autonomous)
2. [x] Propose within the rounds: the settling prompt states `max_rounds` and asks to propose or ask before they run
   out; the last request of a settling offers `propose_verdict` alone, a valid call ends on the proposal, a refused
   one or another tool on the text; verified by agent tests with the fake provider; makes true STL-5, STL-18. (autonomous)
3. [ ] Measure again (paid, cents, after the user's go): same copy and three sections as the ledger's line of
   `e4334139`, usage and tool lines read from the `ui` log; findings section 4 item 5 and the ledger updated.

## Decisions
- Mode: autonomous
- A proposal refused in the last request ends the question on its text, no extra round (1 a).
- The paid measure runs after deliverables 1 and 2, on the user's go then; the agent plays the reader only if asked (2 a).
- Base commit: e6d85b2035a5b4be5796c57dd62119f2b19252ec

## Deliverable 1
done: each settling question logs a second line after its usage line, its tool calls in order with what each reads.

| check | result |
|---|---|
| tests | pass (`settle_page`, workspace) |
| lint | pass |
| specs | pass (STL-19 traced) |
| docs | `docs/crates/render-mcp-cli.md`, `docs/dev/commands.md` |
| code review | see below |

red: STL-19: `one line per settling question: [...] left: 0 right: 2` (no tool line logged)
Code review: no findings
not verified: none
decisions needed: none

## Deliverable 2
done: the settling prompt states the rounds of a question and asks to propose or ask before they run out; the last
request of a settling offers `propose_verdict` alone, a valid call ends on its proposal, a refused one or another tool
on the text. The spec is `accepted` again.

| check | result |
|---|---|
| tests | pass (`retrodoc-agent`, workspace) |
| lint | pass |
| specs | pass (STL-5, STL-18 traced, spec `accepted`) |
| docs | `docs/crates/agent.md` |
| code review | see below |

red: STL-5, STL-18: `left: Some([]) right: Some(["propose_verdict"])` (last request offered no tool); the prompt held
no "You have 3 rounds of tool calls"
Follow-up: `issues/ask_page_session_test_awaits_its_first_question.md`, a flaky test of the ask page met on the way
(2 runs of 15), unrelated to this change.
Code review: 1 finding
- `lib.rs`: a proposal refused in the last request, its message text blank, ended on an empty answer: fixed, the
  answer is then the comment of the refused call (STL-18 amended, test extended).
not verified: what DeepSeek does with the last request (deliverable 3).
decisions needed: none
