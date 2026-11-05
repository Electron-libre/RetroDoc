---
priority: low
depends_on: [reasoning_of_openrouter_and_ollama_given_back]
why: 'the ask agent runs on DeepSeek; a thinking model through OpenRouter is a comfort, and the risks below are unverified'
---
# Check the reasoning given back on a real OpenRouter call

# Goal
`retrodoc-llm` now gives back the `reasoning` and `reasoning_details` of OpenRouter (TCL-17, TCL-18, TCL-20), but
no real OpenRouter call checked it. Expected: one tool loop of `retrodoc ask` through OpenRouter on a thinking model
runs to its answer, without a refused round, without the reasoning sent twice, and within the agent's budget.

# Findings
* 2026-11-06, `high` review of `issues/done/reasoning_of_openrouter_and_ollama_given_back.md`, read from the code, not
  checked on a real call (no OpenRouter key on the machine):
  1. An encrypted `reasoning_details` is given back as received; a round routed to another upstream than the one
     that signed it may answer 400, and nothing sends it again without the reasoning, so `ask`/`chat` fails.
  2. `reasoning` and `reasoning_details` are both given back; if both reach the prompt, the reasoning tokens are
     billed twice per round.
  3. The reasoning given back (encrypted entries, and the `reasoning_content` of DeepSeek) is not counted in the
     budget of the agent, which counts the tool results only.
* 2026-11-06, Ollama 0.40.7 local probe: `reasoning` given back is read (prompt 1,843 tokens against 324 without).

# Approach
1. With an OpenRouter key (paid, cents: ask first), run one tool loop on a thinking model and record which fields
   come back, the prompt tokens of a round with both fields, with `reasoning_details` only, and with none.
2. Depending on what 1 shows, candidates: send `reasoning_details` only when both came; on a 400 naming the
   reasoning or its signature, send the round again without it; count the reasoning given back in the agent's budget.

# Specs
* `specs/tool-calling.md`: TCL-18 if one field is no longer sent back; a new requirement for a retry without the
  reasoning.
* `specs/ask.md`: the budget of the agent, if the reasoning is counted.

# Milestone
Ask the documentation

# Resources
* `specs/tool-calling.md`, `docs/crates/llm.md`, `docs/crates/agent.md`, ADR 0046, ADR 0047,
  `crates/retrodoc-llm/src/openrouter/mod.rs`, `crates/retrodoc-agent/src/lib.rs`.

# Hints
* Read the measures ledger (`LEDGER.md` of the runs directory, outside git) before the paid probe and append a line after.
