# 0057. The agent proposes a verdict through a tool that writes nothing

Status: Proposed

## Context

A reader who doubts a section (reviewed wrong or to review, or contradicted by the project's docs) checks it against
the code and the history alone. The agent of `ask` and `chat` (ADR 0047) already reads a section, its code, its
history and the docs, but only answers: it offers the read-only tools and never writes, and the reviews and edits
are saved only by the form of a section (`specs/section-form.md`). Settling a section needs the agent to ask the
reader what the code cannot tell, then to end on a review and a correction the form can take, field by field,
which a free text answer does not give reliably.

## Decision

This amends ADR 0047 and ADR 0048. The ask page of `ui --ask` gains a settling conversation, started from the form
of a section: the server empties the session, sends a first message built from the docs (the detail answer of the
section, its review, its edit, its contradictions with the project's docs) and the questions of that conversation
go with a system prompt of verification. A question of the agent to the reader is the text of an answer; the reader
answers in the question field, as a follow-up of `chat`, so no turn has to pause. The settling questions offer one
more tool, `propose_verdict` (status, comment, name, description, narrative), which writes nothing: a valid call ends
the question and the proposal is kept with its exchange in memory. The exchange links to the page of the section,
whose form is filled from the proposal; only the reader's save writes the review and the edit.

Rejected: a verdict parsed from a fenced block of the answer text (a model that forgets or breaks the block gives
nothing, and the format would be checked after the turn, too late to have it fixed); a tool that saves the review
and the edit (the agent would write what the reader has not read, against WEB-81 and WEB-93); a page of its own per
section (a second conversation store beside the sessions of ADR 0048, for the same streamed exchange).

## Consequences

`retrodoc-agent` offers a tool that is not a read tool of `retrodoc-docs`, only to a settling conversation: `ask`,
`chat` and the MCP server never see it. Starting a settling drops the exchanges of the ask session, so the button asks
a confirmation when there are some. A proposal lives as long as its exchange: a restart of `ui` loses it, and the
transcript is not kept with the review, the proposed comment carrying the evidence.
