---
priority: low
depends_on: [agent_helps_settle_a_disputed_section]
why: 'the settling works and its verdicts matched the reader''s; this makes them easier to judge and measurable'
---
# Write the proposal of a settling for the reader, and log what a settling costs

# Goal
The agent that settles a disputed section (`specs/section-settle.md`) proposes a verdict the reader saves or not. In the
first measure the reader found the verdicts right overall, but could not judge their details: the comment speaks to a
developer, quibbles over points the reader does not follow, and its status can contradict it. The proposal should be
judged at a glance: a short comment in the reader's words, its evidence behind it, a status that agrees with what the
comment says. The turns, calls, tokens and cost of a settling should also be recorded, so the next measure has them.

# Findings
* 2026-11-25, default smoke repository (a copy of a saved run with two sections reviewed `wrong`), `deepseek-chat`,
  `ui --ask`, three settlings by the project's author (`docs/findings.md`, section 4, item 5).
* Overall the proposals matched the reader's verdict; the reader could not judge every detail.
* The one proposal saved, on a domain, had a comment of about 2,900 characters citing file lines, a commit and rule
  ids, and the status `validated` while the comment says the reviewer's mark was right. Whether the agent or the
  reader picked that status, the record does not tell.
* The turns, calls and tokens of each settling were not measured: the ask page shows them in the browser's session
  only, `ui` logs none of them, and the session is gone with the server.

# Approach
1. Measure first: replay the settling of the saved run with the fake provider's capture, to see whether
   `propose_verdict` was called with `validated` or the reader changed it.
2. The proposal for the reader: a `comment` of a few sentences in business words, and a separate `evidence` field
   (files, commits, the reader's answers) shown folded under it, or saved in the review only on demand.
3. A status consistent with the comment: the settling prompt and the tool's description say which status fits
   (a confirmed defect is `wrong`), or the tool refuses a `validated` whose comment names a correction.
4. Log each settling's usage on the server (`ui --ask`: turns, calls, tokens in and out, cost), in the same form as
   the question of `ask`, so a measure reads it from the log.

# Specs
* `specs/section-settle.md`: amends STL-13 (the arguments of `propose_verdict`) and STL-7 (what an exchange shows); a
  new requirement for the logged usage.

# Milestone
Read by someone taking an application over

# Resources
* `specs/section-settle.md`, ADR 0057, `crates/retrodoc-agent/src/settle.rs` (the prompt and the tool).
* `crates/retrodoc-web/src/ask.rs` and `ask/settle.rs` (the exchange, its cost, the session).
* `docs/findings.md`, section 4, item 5.

# Hints
* The reader of the measure is not a developer of the measured application: judge the method with them, not a list
  of items.
* A paid measure needs the user's go and the measures ledger read before.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-25

## Understanding
A settling's proposal judged at a glance: a short comment for the reader (at most 400 characters), its evidence in a
field of its own folded under it and not saved with the review, a status the tool refuses when it contradicts a
correction (`validated` with a corrected field); each settling question logs its rounds, calls and tokens on `ui`.
Means differs: no replay first (approach 1): the session of 2026-11-25 was not captured and the fake provider
replays scripted answers, not the model's; the log line records the proposed status, so the next measure tells.
Out of scope: the paid measure of the ledger's queue (asked apart, after the issue).

## Plan
1. [x] Proposal in the reader's words: `evidence` argument, comment limit and `validated`-with-correction refusal in
   `propose_verdict`, the settling prompt and the tool's descriptions say which status fits; the exchange shows the
   evidence folded, the form takes the comment alone, the kept answer holds the evidence; verified by agent unit
   tests and `settle_page.rs`; makes true STL-5, STL-7, STL-8, STL-13, STL-15 (~450 lines). (autonomous)
2. [x] Log of each settling question: one `tracing` line at its end (section, question number, rounds, tool calls,
   LLM calls and tokens, proposed status, settling sums), the code in `ask/settle.rs` (`ask.rs` is at 781 lines of
   800: only the call goes there); verified by a web test capturing the log; makes true STL-17 (~200 lines). (autonomous)

## Decisions
- Mode: autonomous
- Base commit: d11a0191d7838c4c060ceeefdb73b41cd41ac261
- Answers (2026-11-25, all recommendations): the form takes the comment alone, the evidence stays in the exchange;
  the tool refuses a comment over 400 characters; it refuses `validated` with a corrected field; only the questions
  of a settling write a log line.

## Deliverable 1: proposal in the reader's words

done: `propose_verdict` takes `evidence`, refuses a comment over 400 characters and `validated` with a corrected
field; the prompt and the tool say how to write the comment and which status fits; the exchange folds the evidence
under the comment, the form takes the comment alone, the kept answer holds the evidence.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (`section-settle` draft until STL-17 is traced) |
| docs | `docs/crates/agent.md`, `docs/crates/render-mcp-cli.md` |
| code review | no findings |

red: STL-5, STL-7, STL-8, STL-13, STL-15: `struct Proposal has no field named evidence` (agent), `settle_page.rs`: no `<details class="evidence">` in the exchange

Code review: no findings

not verified: what the model writes with the new prompt (a paid measure, after the issue).

decisions needed: none

## Deliverable 2: log of each settling question

done: each settling question that ends logs one line on the stdout of `ui` (section, its number in the settling,
rounds, tool calls, LLM calls and tokens, proposed status, the settling's sums); `section-settle` is `accepted`.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (`section-settle` accepted, every requirement traced) |
| docs | `docs/crates/render-mcp-cli.md`, `docs/dev/commands.md` |
| code review | 1 finding, fixed |

red: STL-17: `one line per settling question: []` (no line logged)

Note: the log test installs a global subscriber writing per thread, as a scoped one missed the line when another
test reached the call site first.

Code review: 1 finding
- The question number read the exchanges kept, stuck at 50 past 50: fixed, numbered from `settling_from`, the number
  of the settling's first question, kept when a dropped session is started again; a second review found nothing.

not verified: the line in the log of a real `ui --ask` (a paid measure, after the issue).

decisions needed: none
