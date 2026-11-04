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

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
A `reviews` module of `retrodoc-mcp` that loads, sets, removes and saves the reviews of domains, features and use
cases in `.retrodoc/reviews.json`, with the git author, the date and a fingerprint of the section. Out of scope: any
page, debt, MCP or outdated logic (the other issues of the milestone); nothing calls the store yet but its tests.

## Plan
1. [x] Review store: `reviews` module (model, fingerprint, git author in `retrodoc-ingest`, load/save atomic,
   set/remove), spec `specs/section-review.md`, ADR 0044; verified by unit tests of the module and one CLI test that
   `generate --force` keeps the file; makes true REV-1 to REV-16. (autonomous)
2. [x] Fixes of the `high` review: drafts unique per save and removed on any failure, saves of one process in turn,
   the folder synced after the rename, the newest of duplicate reviews kept, a missing comment read as empty, the
   kind named as in the file, the author found from a subfolder; verified by new unit tests; makes true REV-17,
   REV-18. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 62d0a2691c8abbaa1febdd28298e1ca246d0519e
- Fingerprint: the whole section as saved, confidence included (1 a).
- A review of a section the docs do not hold is refused (REV-16, 2 a).

## Deliverable 1 summary
- done: module `retrodoc_mcp::reviews` (`record`, `remove`, `load`, `fingerprint`), `git_history::user_name`,
  spec `section-review.md` accepted, ADR 0044, docs (ARCHITECTURE §2 and §7, `docs/crates/`).
- red: REV-1, REV-2, REV-3, REV-4, REV-5, REV-6, REV-7, REV-9, REV-10, REV-11, REV-12, REV-13, REV-14, REV-15, REV-16:
  `not yet implemented` (stubs); REV-4 (`user_name`) and REV-8 (`generate` keeps the file) written with or after
  the code, green at once (REV-8 already held: the file is outside the cache).
- Code review: no findings
- not verified: none. The `version` test of `retrodoc-cli` failed on `main` (0.8.0 released without the bump of
  `[workspace.package] version`): fixed in a commit of its own before this one (decided with the user).

## Deliverable 2 summary
- done: the fixes of the `high` review of the issue; spec amended with REV-17 (duplicates of a merged file) and
  REV-18 (saves of one process), plus two scenarios (REV-3, REV-4).
- red: REV-3, REV-4, REV-16, REV-17, REV-18: `missing field comment`, author `None` in a subfolder, `the docs hold no
  Feature ship`, two reviews loaded instead of one, scoped threads panicking on the shared draft.
- Code review: 10 findings (high review of the issue): fixed 1 (unique draft), 2 (one save at a time), 3 (draft
  removed on any failure), 4 (duplicates), 5 (REV-15 test reads the file), 6 (comment default, full CLI fixture),
  7 (kind in the error), 9 (`Repository::discover`), 10 (folder synced); not fixed 8 (reuse the pipeline's
  `artifact::write`): it writes a fixed `<name>.tmp` draft, the very collision of finding 1.
- not verified: none.
