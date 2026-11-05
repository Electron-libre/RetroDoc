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
