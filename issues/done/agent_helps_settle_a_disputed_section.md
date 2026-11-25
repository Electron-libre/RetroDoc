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
2. [x] Web start of a settling: button in the section form with its conditional confirmation, `POST /ask/settle`
   (refusals 400/404/409, reset then first question streamed as WAK-30), the settled section named on `/ask`; code in a new
   `crates/retrodoc-web/src/settle.rs` (texts in `settle_texts.rs`, `texts.rs` being over the file size limit), tests in a
   new `crates/retrodoc-web/tests/settle_page.rs` (~700 lines); makes true STL-1, STL-2, STL-3, STL-10, STL-11, STL-12. (autonomous)
3. [x] Web proposal: the proposal shown in its exchange with "Fill the form", the section page filled from
   `?proposal=<n>` with its notice, nothing written before the save (~500 lines); makes true STL-7, STL-8, STL-9, STL-16;
   sets the spec `accepted`. (autonomous)
4. [x] Measure on 3 disputed sections of the default smoke repository with DeepSeek (paid: asked before the run, ledger
   read and appended): turns, calls, tokens, whether the proposal matches the reader's verdict; a dated entry in
   `docs/findings.md` (~40 lines). (autonomous)
5. [x] Docs of the whole issue and fixes of its `high` review. (autonomous)

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

## Deliverable 2: web start of a settling

done: `form.settle` under the form of a section with `ui --ask` (confirmation over exchanges), `POST /ask/settle`
(400/404/409, the session emptied, the first message sent, answered at once), the settled section named on `/ask`;
the ask sessions shared with the pages (`ask::Probe`); the inline tests of `ask.rs` moved to `ask/tests.rs` (file size).
STL-1 and STL-10 reworded: the button is a form of its own under the section form (no nested form), and the start is
answered at once with `HX-Redirect`/303 to `/ask` rather than as a question (WAK-14 waits for the answer without htmx).

| check | result |
|---|---|
| tests | `project-check` green; `tests/settle_page.rs`: 4 new |
| lint | clippy pedantic clean |
| specs | `check-specs` clean for STL (draft) |
| docs | `docs/crates/render-mcp-cli.md`, `docs/dev/web-ui.md` |
| file size | `texts.rs` was over the limit before (1525 → 1528, one field and two lines of instances): the texts live in `settle_texts.rs` |
| code review | low, see below |

red: STL-1, STL-2, STL-3, STL-4, STL-10, STL-11, STL-12: no settle form on the page (`expect("a settle form")`), `/ask/settle` answered 404.

Code review: 1 findings
- nit, `ask/settle.rs`: a panic building the first message sent an empty message to the model: fixed, answered 500 (`read_failed`) before the session is touched.

not verified: the look of `form.settle` and `p.settling` (no screenshots shot, the CSS is two rules).

decisions needed: none

## Deliverable 3: web proposal

done: an exchange with a proposal shows it (`section.proposal`, "Fill the form" to `<page>?proposal=<n>#review`); the
section page fills its form from the proposal of that exchange of the session (`Probe::form`, notice `p.proposed`, the
fields unfolded), anything else ignored; nothing written before the save; spec `accepted`.

| check | result |
|---|---|
| tests | `project-check` green; `tests/settle_page.rs`: 3 new (STL-7, 8, 9, 16, H1, H2) |
| lint | clippy pedantic clean |
| specs | `check-specs` clean, STL `accepted` |
| docs | `docs/crates/render-mcp-cli.md`, `docs/dev/web-ui.md` |
| code review | low, see below |

red: STL-7, STL-8, STL-9, STL-16: no `class="proposal"` in the exchange; the page with `?proposal=1` had no `class="proposed"`.

Code review: 1 findings
- nit, `ask/settle.rs`: a proposed narrative dropped when the form has no narrative field: not fixed, it cannot happen: only the form of a use case has none missing (`EditFormView::new`), and `propose_verdict` refuses a narrative for a domain or a feature (STL-13), the proposal filling only the form of its own section (STL-16).

not verified: the preview of a narrative filled from a proposal shows the former narrative until the field is typed in (its `hx-trigger` is `input`); the look of `section.proposal` and `p.proposed` (no screenshots).

decisions needed: none

## Deliverable 4: measure

done: settlings by hand on a copy of the saved delivery_router run with two `wrong` reviews (`generate` at HEAD: 0
calls), `deepseek-chat`; entry 5 of section 4 of `docs/findings.md`; ledger line appended.

| check | result |
|---|---|
| tests | none (a measure); `project-check` green |
| specs | `check-specs` clean |
| docs | `docs/findings.md` |
| code review | low, see below |

red: none

Code review: no findings

not verified: the turns, calls and tokens of each settling (shown only in the browser's ask session, not logged by
`ui`); whether the `validated` status saved on the domain was the agent's or the reader's.

decisions needed: none. Follow-up proposed at the final summary: proposals in the reader's words (short comment,
evidence behind it, status consistent with the comment) and the usage of a settling logged.

## Deliverable 5: docs of the issue and fixes of the high review

done: `PRODUCT.md` names the settling; the 6 findings of the `high` review over `5413f8cb..HEAD` fixed: the refusals
of `/ask/settle` answered as plain text, shown in `#page-error` of the section page (STL-11); the settling prompt
names the section (`settling_prompt`) and the tool-less last round asks the verdict as text (STL-5); a session
started again after its drop keeps `Conversation::settling` (`Running::settled`); a proposal unfolds only the fields
it corrects and previews its narrative (`EditFormView::proposed`, STL-8). Spec STL-5, STL-11 amended.

| check | result |
|---|---|
| tests | `project-check` green; new: `the_last_round_of_a_settling_asks_the_verdict_as_text`, `a_settling_session_started_again_still_settles_its_section`, `a_proposal_unfolds_and_previews_only_the_fields_it_corrects`; STL-11 and STL-16 tests extended |
| specs | `check-specs` clean |
| docs | `PRODUCT.md`, `docs/crates/agent.md`, `docs/crates/render-mcp-cli.md` |
| code review | high over the issue: 2 important, 4 nits, all fixed; low of this deliverable below |

red: STL-5, STL-8, STL-11: not observed, the tests were written with the fixes (they assert headers, constants and a
method the former code lacked).

Code review: no findings

not verified: the plain-text refusal in a browser (`#page-error`).

decisions needed: none
