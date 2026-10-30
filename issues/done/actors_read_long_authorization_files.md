---
rank: 500
why: 'first of milestone `A large application`: the actors of a large application are cut at 3,000 characters per file; before `contexts_residual_size_and_names`, which is low'
---
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

# Tracking

Plan: validated

## Understanding
Measured on 2026-10-30 (no LLM, saved trees): no selected file is longer than 3,000 characters on linkding, lobsters,
delivery_router or the large private Rails app. What loses actors there is the 12-file cap: most of its candidate
files are never sent (figures in the ledger, outside git). So: replace both caps by one prompt budget, cut a long file with `Splitter::excerpt`, log what is left unread.
Out of scope: one call per chunk group (candidate b), the file selection itself.

## Plan
1. [x] Read the authorization code within one budget: files taken whole in selection order up to a total budget,
   a file over the per-file budget sent as its excerpt (focus: entity names, authorization words), the files past
   the budget logged as one warning, the fingerprint on the text sent; verified by tests in `actors.rs`; makes true
   ACT-1 to ACT-5.

## Decisions
- Mode: autonomous
- Base commit: 8e015fa
- Budgets: 40,000 characters of authorization code per prompt, 8,000 per file before it is sent as an excerpt (answers 1 a, 2 a).

## Deliverable 1
- done: `actors.rs` reads every authorization file within 40,000 characters (whole up to 8,000, else
  `Splitter::excerpt` focused on entity names and authorization words), warns of the files left unread, fingerprints
  the text sent; spec `specs/actors-reading.md` accepted; `docs/crates/pipeline.md`, `docs/findings.md` updated.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | ACT-1 to ACT-5 traced |
| docs | updated |
| code review | low: no findings |

- red: ACT-1: `file 12 missing`; ACT-2: prompt lacked the last definition (head cut at 3,000); ACT-3, ACT-4:
  `left: 0, right: 1` warnings; ACT-5: already green (reuse on unchanged content), kept as its proof.
- Code review: no findings
- not verified: no run on a real repository (the saved actors of the runs are re-identified once, the fingerprint
  now holds the text sent).
- decisions needed: none.

## Deliverable 2: fixes of the high review
- done: a file the budget left can hold neither whole nor as a 2,000-character excerpt is left unread (no head cut);
  the excerpt is shrunk until its omitted markers fit; every file sent with numbered lines; entity names matched as
  one word and in snake case, first 25 only; `retrodoc actors` says "found"; confidential figures removed from the
  issue and findings.

| check | result |
|---|---|
| tests | pass (`just check`) |
| lint | pass |
| specs | ACT-3 amended, ACT-2 scenario reworded |
| docs | `docs/crates/pipeline.md`, `docs/findings.md` |
| code review | low: see below |

- red: ACT-2, ACT-3: the strengthened tests (total within the budget, last file sent whole, snake-case entity) cover
  the cases the high review found; not run against the code before the fix.
- High review: 9 findings. Fixed: head cut at the budget tail, markers over the budget, mixed numbered/raw formats,
  snake-case entity focus, CLI count, confidential figures. Not fixed: fingerprint re-identification once on
  upgrade and its dependency on the excerpt (wanted: the text sent is the input, issue hint); bytes counted as
  characters (same as every other budget of the pipeline); the loop shared with the confidence pass (different
  rules, a helper would be a refactor of another pass).
- Code review: 1 finding: `first_chars` counts characters, the budget bytes (underflow on non-ASCII); fixed, cut at a byte boundary and `saturating_sub` restored.
- not verified: no run on a real repository.
- decisions needed: none.
