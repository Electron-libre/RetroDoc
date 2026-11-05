---
priority: medium
depends_on: [ask_command]
why: 'tells whether the answers are right and whether the code access helps; a paid run, after the command works'
---
# Measure the answers of `retrodoc ask` on a question set

# Goal
Answers are hard to judge automatically, and the agent may repeat the errors of the generated docs with
confidence (`docs/findings.md` §4, risks). Expected: a set of questions on a benchmark repository whose answers
are known, the answers of `retrodoc ask` scored with and without the code tools, written as dated findings.

# Findings
* 2026-11-05: the benchmark repositories have a hand-written reference of their features and use cases
  (`quality-benchmark` skill), which known answers can be drawn from.
* The search was measured with 42 hand-written questions (21 documented, 21 not) over three saved runs
  (`issues/done/search_relevance_threshold.md`): a precedent for the documented / undocumented halves.

# Approach
1. A method to draw the questions and their expected answers from the reference (documented ones) and from what
   the application doesn't do (undocumented ones), validated with the user before any question is written.
2. A judge of each answer (right, wrong, "not documented"; sources cited exist), by rule where it can, by an LLM
   judge otherwise, in `retrodoc-bench`.
3. One run with the code tools and one without, on the saved run of the benchmark at the current code (reuse it,
   no new `generate`); figures and cost in the ledger and `docs/findings.md`.

# Milestone
Ask the documentation

# Resources
* `.claude/skills/quality-benchmark/`, `docs/crates/bench.md`, ADR 0042, `issues/done/search_relevance_threshold.md`.
* The ledger of the measures (`LEDGER.md` of the runs directory) before the paid run; ask before it.

# Hints
* The user does not know the benchmark applications: validate a method with them, never a list of questions or
  answers to review by hand.
