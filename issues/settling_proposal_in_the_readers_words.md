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
