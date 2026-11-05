---
priority: low
depends_on: [ask_command]
why: 'a comfort over `ask`: follow-up questions without restating the context'
---
# Question the docs in an interactive `retrodoc chat`

# Goal
`retrodoc ask` answers one question; a follow-up must restate its context. Expected: `retrodoc chat` keeps the
conversation, answers each question like `ask` (sources, warnings, "not documented"), and stays within the context
of the model.

# Approach
1. A loop over the agent of `ask_command`, the earlier questions and answers kept in the messages.
2. Bound the memory: the earlier tool results dropped first, then the oldest turns, so a local model's context is
   not exceeded.
3. Tested with a fake provider and a scripted input, like `ask`.

# Specs
* `specs/ask.md` (of `ask_command`) amended, or a spec of its own.

# Milestone
Ask the documentation

# Resources
* `docs/findings.md` §4 (step 4), `issues/done/ask_command.md`.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
`retrodoc chat` answers questions one after the other like `ask`, earlier questions and answers kept as messages
(tool results never resent), the oldest turns dropped past a memory budget that also shrinks the tool budget.
Out of scope: summarising dropped turns, saving a conversation, a line editor. Spec: `specs/chat.md`.

## Plan
1. [x] Conversation in `retrodoc-agent`: a `Conversation` holding the kept turns, `ask` reused through it with an
   empty memory, the fold and the source check widened to the memory; verified by unit tests with `FakeLlm`; makes
   true CHT-2 to CHT-7. (autonomous)
2. [x] `retrodoc chat` in the CLI: a loop over stdin lines, errors printed and the loop going on, recap at the end;
   docs (`docs/crates/agent.md`, `render-mcp-cli.md`, README, `specs/ask.md` non-goal); verified by binary tests
   with a scripted stdin and the fake HTTP server; makes true CHT-1, CHT-8, CHT-9. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: c0cc02ea6fab46df4142051c8e6f39a794ed4c10
- Answers (2026-11-05): memory keeps question + answer text only; sources of kept turns count as shown; settings and recap of the pass `ask`; memory of 10000 characters.

## Deliverable 1 summary
done: `Conversation` in `retrodoc-agent`, `ask` is the same loop with no earlier turn.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass |
| docs | `docs/crates/agent.md` |
| code review | see below |

red: CHT-2, CHT-3, CHT-4, CHT-5, CHT-6, CHT-7, CHT-8: cannot find type `Conversation` in this scope
Code review: 1 finding
- A turn longer than `MAX_MEMORY_CHARS` alone empties the memory: not fixed, CHT-4 requires it (a longer memory would
  eat the budget of the tool results, CHT-5).

not verified: none. decisions needed: none.

## Deliverable 2 summary
done: `retrodoc chat` in the CLI over `Conversation`; docs (README, CLAUDE.md, ARCHITECTURE, render-mcp-cli), spec
`accepted`.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass, CHT-1 to CHT-9 traced |
| docs | README, CLAUDE.md, `docs/ARCHITECTURE.md`, `docs/crates/render-mcp-cli.md`, `specs/ask.md` |
| code review | see below |

red: CHT-1, CHT-8, CHT-9: error: unrecognized subcommand 'chat'
Code review: no findings

not verified: a run on a real model (no paid run asked). decisions needed: none.
