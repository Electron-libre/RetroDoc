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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-26

## Understanding
Log why each call of `propose_verdict` of a settling question is refused (STL-20), measure the settlings again to
read the reasons, then fix the cause found so the first call is accepted in most settlings. The measure of
`e3ae1a41` predates ADR 0058: a `validated` or `wrong` with a corrected field was refused then, so part of its
refusals may already be gone. The fix is chosen after the measure; its deliverable is planned then (a stop).

## Plan
1. [x] Log of the refusals: `Answer` keeps each refused call of `propose_verdict` (status, comment length, error),
   the last request's included, and `ui` writes the STL-20 line after the STL-19 one; verified by agent tests (a
   refusal in a round, in the last request, none) and a web test of the line; makes true STL-20. (autonomous)
2. [x] Measure: the three settlings of `e3ae1a41` again on a copy of the kept run, at this code, the refusals read
   from the `ui` log; recorded in `docs/findings.md` (section 4, item 5) and the measures ledger; verified by the
   recorded figures. (autonomous)
3. [x] The schema of `propose_verdict` states its rules: `comment` has `maxLength` 400, `narrative` is offered for a
   use case only (the tool built for the settled section), and the error of a long comment names its length;
   verified by agent tests of the schema per kind and of the error; makes true STL-13 (amended). (autonomous)
4. [x] Measure again: the same three settlings on a fresh copy, the refusals read from the STL-20 lines; recorded
   in `docs/findings.md` and the ledger; verified by the recorded figures. (autonomous)
5. [x] The limit of the comment goes from 400 to 600 characters (`MAX_COMMENT_CHARS`, the prompt, the schema, the
   error); verified by the agent tests of STL-13 at 600 and 601; makes true STL-13 (amended). Commits the measure
   of deliverable 4. (autonomous)
6. [x] Measure again on a fresh copy, after the user's go; recorded in `docs/findings.md` and the ledger. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 98d11a1f15722a53dc78944adbf7d2c93defc76b
- Measure (deliverable 2): the user runs the three settlings by hand on the sections of `e3ae1a41`, after the paid
  run is confirmed; the agent prepares the copy and reads the log.
- Fix (deliverable 3): planned after the measure, at a stop. Measure of `82272de3`: 3 refusals out of 3
  settlings, 2 a comment over 400 characters (497, 483), 1 a narrative for a feature; none of a status.
- Measure of `1935de14`: 6 refusals, all a comment of 409 to 487 characters (`maxLength` ignored by the model), 3
  in the last request, which lost the question. Answered by the user: the limit goes to 600 characters (no retry
  after the last request), then one more measure with a stop before it.

## Deliverable 2: summary
- done: the fourth measure (`82272de3`, ~$0.03), recorded in `docs/findings.md` (section 4, item 5) and the ledger.
- red: none (a measure).
- Code review: no findings (a doc change only, reviewed with deliverable 3).
- not verified: none.
- decisions needed: the fix of deliverable 3 (plan above).

## Deliverable 3: summary
- done: the schema of `propose_verdict` is built for the settled section: `comment` with `maxLength` 400,
  `narrative` for a use case only; the error of a long comment names its length. Commits the measure of
  deliverable 2 too (`docs/findings.md`).

| check | result |
|---|---|
| tests | pass |
| lint | pass (`project-check`) |
| specs | STL-13 amended and traced, spec `accepted` |
| docs | `docs/crates/agent.md`, `docs/findings.md` |
| code review | `low`: clean |

- red: STL-13: `this function takes 0 arguments but 1 argument was supplied` (`propose_verdict_spec(&section)`), and
  the error of a 401-character comment did not name `401 characters`.
- Code review: no findings
- not verified: whether DeepSeek keeps a comment under `maxLength` (deliverable 4).
- decisions needed: none.

## Deliverable 4: summary
- done: the fifth measure (`1935de14`, ~$0.04), recorded in `docs/findings.md` and the ledger; committed with
  deliverable 5.
- red: none (a measure).
- Code review: no findings (a doc change only, reviewed with deliverable 5).

## Deliverable 5: summary
- done: `MAX_COMMENT_CHARS` is 600 (the prompt, the schema's `maxLength` and the error follow it).

| check | result |
|---|---|
| tests | pass |
| lint | pass (`project-check`) |
| specs | STL-13 amended (600), spec `accepted` |
| docs | `docs/crates/agent.md`, `docs/findings.md` |
| code review | `low`: clean |

- red: STL-13: `a_proposal_needs_its_evidence_a_short_comment_and_a_status_that_fits_its_corrections` and
  `the_last_request_holds_a_proposal_refused_in_a_round_and_its_error` failed on the 400-character error.
- Code review: no findings
- not verified: the comments of a real settling under 600 characters (deliverable 6).
- decisions needed: none.

## Deliverable 6: summary
- done: the sixth measure (`89c30067`, ~$0.02): `propose_verdict` accepted at the first call in 2 settlings of 3,
  each settling proposed within its budget; recorded in `docs/findings.md` and the ledger.
- red: none (a measure).
- Code review: no findings
- not verified: none.
- follow-up: the comments saved grow to the limit (516 to 598 characters); a shorter comment is a matter of the
  prompt, left out of this issue.
