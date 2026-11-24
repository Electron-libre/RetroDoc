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
