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
* 2026-11-06: the OpenRouter account holds no credit and the user puts none on it: the paid probe is dropped and the
  risk stays unmeasured, recorded in `docs/findings.md` §11 with the probe script left ready in the runs directory.

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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-06

## Understanding
Measure whether a `reasoning.encrypted` given back by `retrodoc-llm` makes OpenRouter answer 400 when round 2 lands on
another upstream, and handle the 400 only if it shows. Out of scope: other providers, reasoning outside the tool loop.
The OpenRouter account has no credits (2026-11-06: `total_credits` 0), so the probe needs credits added first.

## Plan
1. [-] Dropped (no OpenRouter credit, user's decision). Paid probe: `probe.py` (runs directory, outside git) takes a provider order for round 2; run it on
   `google/gemini-3.8-flash` (AI Studio, then Vertex) and `openai/gpt-5-mini` (OpenAI, then Azure), round 2 same and
   other upstream, then one `retrodoc ask` on the copy of the saved linkding run; ledger line and finding in this
   issue; verified by the logs (HTTP status, entry types per round).
2. [-] Dropped with 1. Only if 1 shows a 400 naming the reasoning: a requirement of `specs/tool-calling.md` (send the round again
   without the reasoning), its test on a scripted 400 and the code in `crates/retrodoc-llm/src/openrouter/mod.rs`
   (~150 lines); otherwise the issue closes on the finding.

## Decisions
- Mode: autonomous
- Base commit: 38536ea9c7d2044af60456d15da0423c1624b9cc
- Probe funding: none; the user puts no credit on OpenRouter (2026-11-06). The issue closes unmeasured, the risk is
  recorded in `docs/findings.md` §11 (user's choice over an `on_hold` or an unmeasured retry).

## Summary
- done: the unmeasured risk and the ready probe recorded in `docs/findings.md` §11; no code change.
- checks: project-check pass; code review low: no findings.
- red: none (no test, a docs-only close).
- Code review: no findings
- not verified: the 400 itself (no paid probe).
