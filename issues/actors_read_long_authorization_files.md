# Read the long authorization files whole in the actors pass

# Goal
The actors pass sends at most 12 authorization files (abilities, policies, roles) and cuts each one at 3,000
characters. On a large application the central authorization file is often far longer, so the roles it defines past
the cut never reach the LLM and the use cases lose their business actors. Every other pass that reads long files
already cuts them at their definitions and shows the relevant chunks. Expected: the actors pass reads a long
authorization file through the same chunking, within a prompt budget, and says when it still leaves part of it.

# Findings (code read on 2026-10-30)
* `crates/retrodoc-pipeline/src/actors.rs:33`: `MAX_AUTH_FILE_CHARS = 3_000`; `truncate_chars(content,
  MAX_AUTH_FILE_CHARS)` at line 307; `MAX_AUTH_FILES = 12`.
* The entry points and glossary passes chunk long files (`chunks/`, tree-sitter or the boundary regexes), and the use
  cases and confidence passes use `Splitter::excerpt` (`docs/findings.md` §2 step 3, §5).
* Not measured: how long the authorization files of the reference repositories are, and how many actors the cut
  loses.

# Approach
1. Measure first: the length of the files the pass selects on the reference repositories and the large application
   (saved runs, no LLM call), and whether actors appear past 3,000 characters.
2. Candidates: (a) send each file as an excerpt of its definition chunks (role names, `can`/`authorize` lines first)
   within a total budget; (b) one call per chunk group, actors merged by name; (c) keep the cut but log the part left
   unread, as the entry points pass does.

# Milestone
A large application, one bounded context at a time

# Resources
* `crates/retrodoc-pipeline/src/actors.rs`, `crates/retrodoc-pipeline/src/chunks/`, `Splitter::excerpt`.
* ADR 0009 (business narrative and actors), ADR 0027 (cut long files at their definitions).
* `docs/findings.md` §2 step 4c.

# Hints
* The actors are reused while their input hash is unchanged: the new reading must enter the fingerprint.
* Keep one prompt text for every stack (ADR 0036): no Rails-specific selection of lines.
