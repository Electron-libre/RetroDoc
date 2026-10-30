---
id: ACT
status: accepted
adrs: [0009, 0027, 0036]
---

# Reading of the authorization code by the actors pass

## Intent

The actors pass names who uses the application from its authorization code (the files whose name says ability,
policy, role, permission…) and its entities. A large application spreads its roles over dozens of such files and
sometimes one long central file: the pass must read all of them within one prompt budget, cut a long file at its
definitions rather than at a character count, and say what it still leaves unread, so that the use cases do not
lose their business actors without a trace.

## Requirements

- **ACT-1** (deterministic) The actors prompt must hold every authorization file found, whole, when their total
  length fits the prompt budget of the authorization code, whatever their number.
- **ACT-2** (deterministic) An authorization file longer than the per-file budget must be sent as an excerpt of its
  chunks (`Splitter::excerpt`: the first chunk, then the chunks that name an entity of the glossary or an
  authorization word), with each omitted range of lines marked, not as its first characters.
- **ACT-3** (deterministic) The pass must not send more authorization code than the prompt budget: the files are
  taken in their order of selection (most telling name word first, then path) while the budget allows, and a file
  the budget left can hold neither whole nor as an excerpt of about one chunk is left unread, not cut to its head.
- **ACT-4** (deterministic) The run must log a warning naming the number of authorization files left unread and the
  first of them when the budget leaves some out.
- **ACT-5** (deterministic) The actors must be identified again when the text sent for the authorization files
  changes, and reused while it is unchanged.

## Scenarios

- (ACT-1) Given 20 short policy files, when the actors are identified, then the prompt holds the 20 files, not 12.
- (ACT-2) Given a 12,000-character roles file whose last definition, `shift_approver_can_approve?`, names the entity
  "ShiftApprover", then the prompt shows its first chunk and that definition, and marks the lines in between as
  omitted.
- (ACT-3, ACT-4) Given more authorization code than the budget, then the files past it are not in the prompt and one
  warning gives their count and the first of them.
- (ACT-5) Given a saved actors list, when an authorization file changes, then the LLM is asked again; when nothing
  changes, it is not.

## Non-goals

- More than one LLM call for the actors (one call per group of chunks, actors merged by name).
- A selection of lines specific to a stack or framework (ADR 0036): the excerpt favours entity names and the
  generic authorization words of the file selection.
- A change of which files are selected as authorization code.
