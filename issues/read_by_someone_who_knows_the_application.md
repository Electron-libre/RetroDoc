---
priority: high
why: 'the milestone has no other way to reach its goal, and the users of PRODUCT.md stay assumed until it is done'
---
# Have the generated docs of an application read by someone who knows it

# Goal
Only the author has read generated docs, and the benchmark measures likeness to references written by agents, on
applications the reader does not know. Expected: the generated docs of an application the reader knows well, read
domain by domain, with what is right, wrong, missing and useless written as dated findings in `docs/findings.md`,
then the issues those findings call for.

# Findings
* 2026-11-07, project review: the six releases 0.8 to 0.13 served the consumption of the docs (web interface,
  review, edit, history, ask); the two milestones that test the claims of `PRODUCT.md` were on hold.
* The benchmark references of linkding and Lobsters rest on agents; the reader does not know these applications, so
  a human pass over them can only check a method, not the items.
* A large private Rails application the user knows has complete saved runs (2026-10-15/16, 762 use cases) and
  context map runs up to 2026-11-04 (see the ledger); no saved run is at the current code.
* The web interface already records a verdict per section (`specs/section-review.md`): validated, wrong or to review,
  with a comment.

# Approach
1. Pick the application with the user: one they know in its business detail (the large Rails application above, or
   the default smoke repository), and the bounded contexts to read if it is large.
2. Reuse a saved run if the ledger has one at a close enough code; otherwise one `generate` at HEAD, with the
   estimate shown and the user's go before the paid run.
3. The user reads the docs in `retrodoc ui` and marks each section with the review form; the agent prepares only a
   reading order and a short grid (right, wrong, missing, useless, what they would have wanted first).
4. Turn the reviews into findings: counts per verdict and per kind of section, examples described generically,
   compared with the confidence scores (do the low scores point at the wrong sections?).
5. Propose the issues the findings call for and the change of the "Users" and "Risks" of `PRODUCT.md`.

# Milestone
Read by someone taking an application over

# Resources
* `PRODUCT.md` (Users, Success measures, Risks), `docs/findings.md`.
* `specs/section-review.md`, `docs/dev/web-ui.md`.
* Skills `smoke-test`, `quality-benchmark`; the measures ledger.

# Hints
* Confidentiality: the findings name no private application; figures and generic descriptions only.
* The user's time is the cost here: keep the grid short and let the review form hold the detail.
