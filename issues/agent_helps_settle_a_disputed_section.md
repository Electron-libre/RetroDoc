---
priority: low
depends_on: [one_form_to_review_and_edit_a_section]
why: 'a new capability for the review of a disputed section; the reading can be done without it'
---
# Let an agent help settle a disputed section

# Goal
When a user reviews a section they doubt (marked wrong or to review, or a disagreement with the project's docs),
they check the code and the history alone. Expected: from the review of that section, the user asks an agent to
help; the agent checks the section against the code, the history and the project's docs, then asks the user the
questions that would settle it, and the exchange ends in a proposed verdict and correction the user accepts or not.

# Findings
* The agent of `ask` and `chat` (ADR 0047) is a bounded tool loop over read-only tools that already read a section,
  its sources (`read_source`), the history (`git_log`) and the search (`crates/retrodoc-docs/src/tools/`); its
  detail answers say the review and the edit of a section.
* The web interface asks the docs only with `ui --ask` (ADR 0048), its steps streamed (ADR 0049).
* The agent never writes: reviews and edits are saved only by the forms (WEB-81, WEB-93).

# Approach
1. Shape the exchange with the user: what starts it (a button in the review of a section), what the agent receives
   (the section, its review comment, its cited code, the disagreement with the docs), how it asks (questions with
   choices, free text), how it ends (a proposed status, comment and edit filled in the form, saved only by the user).
2. Candidates (can be combined):
   1. a page or a panel of a section reusing the ask page and its streamed steps, seeded with the section;
   2. a prompt of the agent for verification: check each claim of the section against the code, list what it
      could not settle, then ask;
   3. the transcript kept with the review comment, as evidence.
3. Measure on a few disputed sections of the default smoke repository: turns, cost, whether the proposed verdict
   matches the user's.

# Specs
* `specs/web-ask.md`, `specs/ask.md`, `specs/section-review.md`; a new spec if the exchange is a capability of its
  own.

# Milestone
Read by someone taking an application over

# Resources
* ADR 0047, 0048, 0049; `docs/crates/agent.md`, `crates/retrodoc-web/src/ask.rs`.

# Hints
* Each question costs LLM calls: show the cost as the ask page does (WAK) and ask before paid measures.
* A loop that asks the user needs the turn to pause on the agent's question, unlike `ask` which always answers.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-24

## Understanding
From the form of a doubted section (`ui --ask` only), a button starts a settling conversation on the ask page: a first
message built from the docs (section, review, edit, contradictions), a verification prompt, the agent's questions answered
as follow-ups, and a `propose_verdict` tool that writes nothing; the proposal fills the form, saved only by the reader.
Out of scope: transcript kept with the review, choice buttons, settling from `chat`. Spec `specs/section-settle.md`
(STL-1 to STL-16), ADR 0057.

## Plan
1. [x] Agent settling conversation: `Conversation::settling` with the settling message of a section (detail answer +
   contradictions), the verification system prompt, the `propose_verdict` tool and `Answer::proposal`, the proposal in the
   kept turn; tests in a new `crates/retrodoc-agent/src/settle_tests.rs` with the scripted `FakeLlm` (~650 lines); commits
   the spec and ADR 0057 drafts; makes true STL-4, STL-5, STL-6, STL-13, STL-14, STL-15. (autonomous)
2. [ ] Web start of a settling: button in the section form with its conditional confirmation, `POST /ask/settle`
   (refusals 400/404/409, reset then first question streamed as WAK-30), the settled section named on `/ask`; code in a new
   `crates/retrodoc-web/src/settle.rs` (texts in `settle_texts.rs`, `texts.rs` being over the file size limit), tests in a
   new `crates/retrodoc-web/tests/settle_page.rs` (~700 lines); makes true STL-1, STL-2, STL-3, STL-10, STL-11, STL-12.
3. [ ] Web proposal: the proposal shown in its exchange with "Fill the form", the section page filled from
   `?proposal=<n>` with its notice, nothing written before the save (~500 lines); makes true STL-7, STL-8, STL-9, STL-16;
   sets the spec `accepted`.
4. [ ] Measure on 3 disputed sections of the default smoke repository with DeepSeek (paid: asked before the run, ledger
   read and appended): turns, calls, tokens, whether the proposal matches the reader's verdict; a dated entry in
   `docs/findings.md` (~40 lines).

## Decisions
- Mode: autonomous
- Base commit: 5413f8cbfc82d536c426eb93bd2e7288f97feb15
- Session: a settling empties the ask session, with a confirmation when it holds exchanges (1 a).
- Proposal: the form is filled from `?proposal=<n>`, read from the session (2 a).
- Measure: the last deliverable, the paid run asked before (3 a).

## Deliverable 1: agent settling conversation

done: `Conversation::settling` and module `settle` of `retrodoc-agent` (settling message, verification prompt,
`propose_verdict`, `Answer::proposal`, the proposal kept as text); spec `section-settle.md` and ADR 0057 drafts.

| check | result |
|---|---|
| tests | `cargo test -p retrodoc-agent`: 29 passed, 5 new |
| lint | clippy pedantic clean |
| specs | `check-specs` clean for STL (draft) |
| docs | `docs/crates/agent.md`, `docs/ARCHITECTURE.md` |
| code review | low, see below |

red: STL-4, STL-5, STL-6, STL-13, STL-14, STL-15: the tests did not compile before the code (no `Conversation::settling`, `Disputed`, `Answer::proposal`).

Code review: no findings

not verified: none

decisions needed: none
