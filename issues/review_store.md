---
priority: high
why: 'every other issue of the milestone reads or writes the reviews it stores'
---
# Store the human reviews of the docs sections

# Goal
Nothing records what a human has checked in the generated docs: the confidence score is the model's view. Expected:
a review of a domain, feature or use case (validated, wrong or to review, with a comment, its author and its date),
kept in a file of `.retrodoc/` outside the cache, read and written by the other crates, so the web form, the debt,
the dashboard and the MCP answers share one record.

# Findings
* 2026-11-04: ADR 0039 says the web interface writes nothing "until the review and edit milestones"; this is the
  first write of RetroDoc outside a run.
* 2026-11-04: a section is already identified across runs by its kind and id (the slug, `<feature>/<use case>` for a
  use case: RHI-7 of `specs/run-history.md`).
* 2026-11-04: the judgement of the benchmark lives in `.retrodoc/benchmark/judge.yaml`, outside the cache, so that
  `generate --force` keeps it.

# Approach
1. A model in `retrodoc-mcp` (a `reviews` module next to `history`): a review per section, keyed by kind and id, with
   a status (`validated`, `wrong`, `to_review`), a comment, the author (the `user.name` of the repository's git
   config, none when unset), the date, and a fingerprint of the section as it was reviewed (what issue
   `review_outdated` compares).
2. One file, `.retrodoc/reviews.json` (outside the cache: kept by `generate --force`, versionable by a team), written
   whole aside then renamed, like the run snapshots; an unreadable file is reported, never overwritten.
3. A new review of a section replaces its previous one; removing a review is a review back to no status.

# Specs
* `specs/section-review.md` (new): the model, the file, the author, the replacement.

# Milestone
Human review of the docs

# Resources
* ADR 0039 (the web interface), ADR 0043 (one snapshot file per run: atomic write, format version).
* `crates/retrodoc-mcp/src/history/mod.rs` (the section ids, the atomic save).
* A new ADR: the first write of the web interface and where the reviews live.

# Hints
* Decided with the user on 2026-11-04: the file is outside the cache; the author is the git `user.name`; only
  domains, features and use cases are reviewed (not the rules nor the glossary).
