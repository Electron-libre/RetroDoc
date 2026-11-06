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

* 2026-11-06, free OpenRouter probe at bc6e6f6 (cost 0; logs in `reasoning_openrouter_probe/` of the runs
  directory): round 1 with a tool, then round 2 sent with both fields, `reasoning_details` only, `reasoning` only, none.
  | model (upstream) | details types | prompt tokens: both / details / reasoning / none |
  |---|---|---|
  | `nvidia/nemotron-3-ultra-550b-a55b:free` (Nvidia) | `reasoning.text` | 353 / 353 / 353 / 326 |
  | `nvidia/nemotron-3.5-super-vl:free` (Nvidia) | `reasoning.text` | 358 / 358 / 358 / 326 |
  | `xingchen-agi/xing4.0-29b-a4b:free` (Phala) | `reasoning.text` | 294 / 294 / 294 / 259 |
  | `dots-studio/dots-3-note-preview:free` (AtlasCloud) | `reasoning.text` | 213 / 213 / 213 / 213 |
  1. Risk 2 refuted on these four: OpenRouter sends the reasoning once when both fields come back (same tokens as
     either field alone); every variant got HTTP 200.
  2. Risk 1 not verifiable for free: no free model with tools sends `reasoning.encrypted` (the signed reasoning of
     Anthropic, OpenAI, Gemini is on paid models only; `google/gemma-4-31b-it:free` answered 429).
  3. `retrodoc ask --path <copy of hidden-1> "How does a user share a bookmark with other users?"` on
     `nemotron-3-ultra:free`: 8 tool rounds and an answer with 9 sources, no refused round, 9 calls, 27,161 in /
     1,388 out, 29 s. Risk 3 small here: ~120-150 characters of reasoning a round against a budget of tool results in
     the tens of thousands.

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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Check on one real OpenRouter tool loop of `retrodoc ask` (thinking model) the three risks of the findings: a 400 on
an encrypted `reasoning_details`, the reasoning billed twice, the reasoning outside the agent's budget. Out of scope:
any fix before the probe says one is needed; the fixes are planned after it, at a second stop.

## Plan
1. [x] Probe: one `retrodoc ask` on the saved linkding run through OpenRouter on a thinking model whose
   `reasoning_details` hold encrypted entries, then its second round sent again by hand three ways (both fields,
   `reasoning_details` only, none); verified by the answer reached without a refused round and the `prompt_tokens`
   of each variant; recorded in this issue's Findings and the measures ledger (`docs/findings.md` stops at 2026-10-14: later
   findings live in the issues). No code change. (autonomous)
2. [ ] Fixes the probe calls for (candidates of the Approach: send `reasoning_details` only when both came; a retry
   without the reasoning on a 400 naming it; the reasoning counted in the agent's budget), each with its spec
   amendment (TCL-18, a new TCL requirement, `specs/ask.md`); planned and validated at a second stop after 1.

## Decisions
- Mode: autonomous
- Base commit: bc6e6f6af906cdff137b66c7acb6c85dd89aab7d
- Key: `OPENROUTER_API_KEY` in `.env` (user, 2026-11-06).
- Model: a free OpenRouter model (`:free`, no credits), thinking and with tools; risk 1 needs encrypted entries,
  which the free (open-weight) models may not send: if none does, risk 1 stays unverified and is said so.
- Deliverable 1 done: risk 2 refuted, risk 3 small on open-weight models, risk 1 unverifiable without a paid model
  sending encrypted reasoning. Deliverable 2 waits for the user: no fix is called for by what was measured.
