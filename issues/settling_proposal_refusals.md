---
priority: medium
why: 'the refusals of propose_verdict are where the rounds of a settling go now, and their reason is not measured'
---
# Make a settling's proposal accepted at its first call

# Goal
In the third settling measure, `propose_verdict` was refused before it was accepted in each of the three settlings,
and the first question of the feature spent four of its eight rounds on refused calls. The reason of a refusal is
not logged, so the fix cannot be chosen. A settling should log why each proposal was refused, then get its
proposal accepted at the first call in most settlings.

# Findings
* 2026-11-26, default smoke repository (a copy of a saved run), `deepseek-chat`, `ui --ask`, three settlings by the
  project's author, at `e3ae1a41` (`docs/findings.md`, section 4, item 5, third measure).
* Calls of `propose_verdict` before the accepted one: 1 at the second question of the domain, 1 at the second
  question of the use case, 4 in the first question of the feature (8 rounds, accepted in the last request).
* The tool line of STL-19 names each call of `propose_verdict`, not its result.

# Approach
1. Measure first: log the error of each refused call of `propose_verdict` (with the STL-19 line, or a line of its
   own), then read it on the kept copy of the measure or a new one.
2. Candidate fixes, depending on the reasons found:
   1. A comment over 400 characters: say the limit again in the error with the length, or move the limit to the
      schema of the tool (`maxLength`).
   2. `validated` with a corrected field: no longer refused since ADR 0058, nor `wrong` with one.
   3. A refusal the reader would not have made: reconsider the rule of STL-13 (ADR 0058).
3. Measure again on the same copy and sections, the refusals read from the `ui` log.

# Specs
* `specs/section-settle.md`: STL-19 (or a new requirement) for the logged reason; STL-13 if a rule changes.

# Milestone
Read by someone taking an application over

# Resources
* `specs/section-settle.md` (STL-13, STL-18, STL-19), `crates/retrodoc-agent/src/settle.rs` (`proposal_of`,
  `last_answer`), `crates/retrodoc-web/src/ask/settle.rs` (`tools_line`).
* `docs/findings.md`, section 4, item 5; the measures ledger line of `e3ae1a41`.

# Hints
* A paid measure needs the user's go and the measures ledger read before.
