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
