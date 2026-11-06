---
priority: low
depends_on: [reasoning_given_back_on_a_real_openrouter_call]
why: 'the ask agent runs on DeepSeek; only a paid thinking model through OpenRouter sends encrypted reasoning, and it costs credits'
---
# Check an encrypted reasoning given back on a paid OpenRouter model

# Goal
`retrodoc-llm` gives back a `reasoning_details` as received (TCL-20). An encrypted entry (`reasoning.encrypted`,
signed by the upstream) given back on a round that OpenRouter routes to another upstream may answer 400, and nothing
sends the round again without it, so `ask`/`chat` would fail. Expected: one tool loop of `retrodoc ask` on a paid
model that sends encrypted reasoning runs to its answer, or the 400 is seen and handled.

# Findings
* 2026-11-06, free OpenRouter probe of
  [reasoning_given_back_on_a_real_openrouter_call](done/reasoning_given_back_on_a_real_openrouter_call.md): four free
  thinking models with tools send `reasoning.text` only; none sends an encrypted entry, so this risk is unmeasured.
  The two other risks of that issue were settled there (reasoning billed once when both fields come back; small
  against the agent's budget).

# Approach
1. With OpenRouter credits (paid, cents: ask first), run the probe script of the free probe (`scripts/probe.py` of
   `reasoning_openrouter_probe/` in the runs directory) on a model whose `reasoning_details` hold
   `reasoning.encrypted` (OpenAI, Anthropic or Gemini through OpenRouter), round 2 forced to another upstream
   (provider routing) and to the same one; then one `retrodoc ask` on the copy of the saved linkding run.
2. If a round answers 400 naming the reasoning or its signature: send the round again without the reasoning.

# Specs
* `specs/tool-calling.md`: a new requirement for the retry without the reasoning, if 1 shows the 400.

# Milestone
Ask the documentation

# Resources
* `specs/tool-calling.md`, `docs/crates/llm.md`, `crates/retrodoc-llm/src/openrouter/mod.rs`,
  `crates/retrodoc-agent/src/lib.rs`, ADR 0046.

# Hints
* Read the measures ledger (`LEDGER.md` of the runs directory, outside git) before the paid probe and append a line
  after; the free probe has its line there.
