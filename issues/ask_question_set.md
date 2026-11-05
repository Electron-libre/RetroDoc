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

# Specs
* `specs/ask-question-set.md` (AQS, new), `specs/ask.md` (ASK-21 added: the agent without the code tools).

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-05

## Understanding
A question set on linkding with known answers, the answers of `retrodoc ask` judged by rule and by an LLM judge, with
the seven tools then without `read_source`/`git_log`, on a saved run (no `generate`); figures as dated findings.
Out of scope: changing the agent or its prompt from the results (later issues), lobsters, the `chat` command.

## Method of the questions
* **Documented** (one per reference feature not `disputed`): a question in the user's words, drawn from the
  feature's description or one of its use cases, without the feature's name words where it can; `expected`: the
  description condensed to what answers the question; `feature`: the reference name. The saved run then splits them
  by rule (AQS-5): `covered` (the docs hold the feature) and `uncovered` (only the code holds it: where the code tools
  may help).
* **Undocumented** (as many): a capability common to the category (bookmark managers, self-hosted web apps) that
  linkding does not have, asked as naturally as the documented ones; `expected`: the application does not do it;
  `absent_terms`: the distinctive words of that capability, which the check (AQS-3) greps in the checkout at the
  pinned commit; a question whose term appears is rewritten or dropped. No human review of the items: the reference
  feature (AQS-2) and the grep (AQS-3) are the checks.
* **Judge**: "Not documented" by rule (AQS-6); otherwise right / wrong by one LLM call (AQS-7); the source hit by
  rule from the run's pairs (AQS-8).

## Plan
1. [x] Question set and its check: `benchmark/linkding/questions.yaml` written by the method above, its loader and
   check in `retrodoc-bench` (`benchmark/questions.rs`), example `question_set` (check only, no LLM); verified by unit
   tests and the check passing on the checkout; makes true AQS-1 to AQS-4, AQS-12. ~700 lines. (autonomous)
2. [ ] The agent without the code tools (`retrodoc-agent`: the tools offered and the prompt follow a flag) and the
   runner with its judge in `retrodoc-bench` (`benchmark/ask_replay.rs`, example `ask_replay <run> <questions>
   <model>`: both variants, answers and judgements saved as YAML, the table printed); `retrodoc-bench` gains
   `retrodoc-agent`, `retrodoc-mcp`; verified by unit tests with `FakeLlm`; makes true ASK-21, AQS-5 to AQS-11, spec
   `accepted`. ~800 lines.
3. [ ] The paid run (estimate shown, go asked first): both variants on the saved run, figures and cost in the
   ledger and `docs/findings.md` §4. ~60 lines.
4. [ ] Docs of the whole issue and fixes of its `high` review.

## Decisions
- Mode: autonomous (deliverable 3 stops anyway for the go of the paid run)
- Base commit: ed16433e3f74fc8daeab2bc829b8ade985628783
- Method of the questions as written above (1a).
- 22 documented (the features not `disputed`) + 22 undocumented (2a).
- Saved run: `hidden-1` of f95ada6, `~/retrodoc-runs/retrodoc-benchmark-linkding/clones-1793722465/hidden-1` (3a).
- One run per variant (4a).
- ASK-21 (spec `ask.md`) is added with deliverable 2, which traces it: an accepted spec can't hold an untraced
  requirement at a commit.

## Deliverable 1 summary
- done: `benchmark/linkding/questions.yaml` (22 documented, one per feature not disputed; 22 undocumented with
  their absent terms), `QuestionSet` and `check_questions` (`retrodoc-bench`, `benchmark/questions.rs`), example
  `question_set`; the check passes on the checkout at 27b7303 (`totp`, in a test configuration of the checkout,
  was left out of the two-factor terms). Spec `ask-question-set` drafted.
- checks: tests ok, clippy ok, specs ok (AQS draft), docs updated (`docs/crates/bench.md`, `docs/dev/commands.md`).
- red: AQS-1, AQS-2, AQS-3, AQS-4, AQS-12: the tests were written with the code; the whole-word test failed first on
  its own fixture (`left: [] right: ["`otp`: `TOTP` appears in `settings.py`"]`: `USE_TOTP` is not a whole word).
- Code review: 1 finding. A blank `commit` of the reference would make the commit check pass any tree: not fixed,
  `Reference::load` refuses a blank `commit` before the check runs.
- not verified: nothing.
