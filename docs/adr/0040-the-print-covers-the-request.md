# 0040. The print covers the request

Status: Proposed

## Context

ADR 0037 put the system prompts in the prints of the LLM passes, but left out the rest of what a request
carries: the JSON schema of the answer type (`response_format`) and the fixed text of the user prompts, spread
in `format!` calls. A new answer field, or an instruction written in a user prompt (the features slice prompt,
the actors filter, the framing of the product brief), kept the saved answers of the old request until
`--force`. The prompt parts were lists kept by hand (`batched_read::reading_instructions` named nine
constants of three modules), and the rules pass kept its own hash of its three prompts, one print for the
statement and the judgement: a change of the judgement alone stated the rules of every feature again
(about 280k prompt tokens on linkding) where judging the saved statements again would do.

## Decision

Each LLM pass declares its request once: its system prompt, its answer type and the fixed text of its user
prompt as constants. The call takes the request, so what is sent and what is printed come from the same
place; the prompt part of the print is the hash of the system prompt, of the schema JSON of the answer type and
of the user prompt texts, computed once per process. The framing of the brief goes with its print part.

The rules pass moves to that part and splits its print: one for the statement (with the question on the kind
of feature), one for the judgement. A feature whose statement print matches and whose judgement print does
not is judged again from its saved statements: the rules it kept (`rules-stated.yaml`) and those the judgement
set aside, which keep their whole rule.

Rejected: hashing the user prompt actually sent: it is built after the print is compared and is cut in parts
by size, so its print would move with the cut. Rejected too: rendering each user prompt from a sample input to
hash its fixed text: a branch the sample misses is left out silently.

Amends ADR 0037 on what the prompt part covers and on the rules pass.

## Consequences

The first run after this change asks every pass of every saved run again once, the rules included; a later
change of the schema or of a user prompt text redoes its pass once. A text of a user prompt written inline
rather than as a constant of its request is still missed: the review of a prompt change checks it. A change
of the judgement prompt costs one short call per feature instead of a statement.
