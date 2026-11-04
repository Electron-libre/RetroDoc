---
priority: medium
depends_on: [review_store]
why: 'the reviews pay off when the debt and the dashboard tell what a human checked'
---
# Count the human reviews in the debt and on the audit page

# Goal
The debt view, the heatmap and the debt report of `retrodoc report` rank the sections by the model's judgement only.
Expected: a section a human validated leaves the debt of judgement, one marked wrong or to review enters it, and the
audit page shows the share of the sections a human reviewed.

# Findings
* 2026-11-04: the kinds of the debt that judge a section are low confidence, not scored, unbacked narrative and
  technical language (`DebtKind` in `crates/retrodoc-mcp/src/debt/mod.rs`); the others are facts of the code or of
  the project's docs (stale, contradicted, undocumented, the code no doc covers).

# Approach
1. A validated section, unchanged since its review, leaves the four kinds of judgement (decided with the user on
   2026-11-04); stale and the kinds of the project's docs still apply.
2. Two new kinds: "marked wrong" (weight of the highest tier) and "to review", with the comment as the reason.
3. The audit page: the share of the domains, features and use cases reviewed, by status; the debt report of
   `retrodoc report` lists the sections marked wrong.

# Specs
* `specs/debt-view.md`: the new kinds, what a validated section leaves.
* `specs/audit-dashboard.md`: the share of the reviewed sections.

# Milestone
Human review of the docs

# Resources
* `crates/retrodoc-mcp/src/debt/`, `crates/retrodoc-mcp/src/audit/`, `crates/retrodoc-web/src/debt.rs`,
  `audit.rs`.
* The run snapshot (`specs/run-history.md`, RHI-5) counts the debt per kind: a new kind changes what a snapshot
  holds.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-04

## Understanding
A validated section, unchanged since its review (same fingerprint), leaves the four kinds of judgement of the debt;
"marked wrong" and "to review" become kinds of the debt (heatmap columns, snapshot counts); the audit page shows the
share reviewed per kind of section; `retrodoc report` lists the sections marked wrong. Out of scope: outdated
reviews (`issues/done/review_outdated.md`), the lists of `coverage-report.md`.

## Plan
1. [x] Debt with the reviews: `debt` takes the reviews, validated sections leave the kinds of judgement, kinds
   "marked wrong" and "to review" (domain items included), the debt page reads the reviews at each request and says
   when they can't be read, the heatmap gets their columns; verified by tests of `/debt` and `/audit`; makes true
   DBT-8, DBT-29 to DBT-34, AUD-6 (~700 lines). (autonomous)
2. [x] Snapshot: the debt counted by a `generate` takes the reviews into account; verified by a test of the history;
   makes true RHI-14 (~150 lines). (autonomous)
3. [x] Audit page: the share of domains, features and use cases reviewed, by status; verified by tests of `/audit`;
   makes true AUD-16, AUD-17 (~350 lines). (autonomous)
4. [x] `retrodoc report` lists the sections marked wrong; verified by a test of the binary; makes true DBT-35,
   DBT-36 (~200 lines). (autonomous)
5. [x] Docs of the whole issue and fixes of its `high` review: README, one helper for the sections of the run a
   review names, the French texts of the audit page; verified by the tests of deliverables 1 to 4. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 230236091dbd58d4004962ddcfb47383aa98cc17
- The snapshot counts the debt with the reviews (RHI-14).
- "to review" weighs 0.5 (DBT-29).
- A section marked wrong or to review stays in the debt when its section changed since (DBT-30, DBT-31).

## Deliverable 1
done: the debt takes the reviews: validated sections leave the kinds of judgement, kinds "marked wrong" and "to
review" (domains included), the debt page reads the reviews at each request, the heatmap has their columns.

| check | result |
|---|---|
| tests | pass (`cardinal run project-check`) |
| lint | pass |
| specs | pass (debt-view, audit-dashboard, run-history in `draft`) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | no findings |

red: DBT-32, DBT-30, DBT-31, DBT-33, DBT-29, DBT-8, DBT-34, AUD-6: the validated use cases still listed as low
confidence and not scored, no item "Marked wrong" nor "To review", no note of an unreadable `reviews.json`, no
heatmap column "Marked wrong".

Code review: no findings
not verified: none
decisions needed: none

## Deliverable 2
done: the snapshot of a `generate` counts the debt with the reviews `reviews.json` holds when the run ends; an
unreadable file is warned of and counts no review.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (run-history `draft`) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | no findings |

red: RHI-14: a rerun after a review wrote no snapshot (1 file, expected 2); no warning for an unreadable
`reviews.json`.

Code review: no findings
not verified: none
decisions needed: none

## Deliverable 3
done: the audit page shows, for the domains, features and use cases of the run, the share a human reviewed and the
count of each status; an unreadable `reviews.json` is said in its place.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (audit-dashboard `draft`) |
| docs | `docs/crates/render-mcp-cli.md` |
| code review | no findings |

red: AUD-16, AUD-17: no section `audit-reviews` on the audit page.

Code review: no findings
not verified: none
decisions needed: none

## Deliverable 4
done: `retrodoc report` lists the sections of the run marked wrong with their comment, under "Sections marked
wrong" when one is; an unreadable `reviews.json` is warned of. The three specs are `accepted`.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass (debt-view, audit-dashboard, run-history `accepted`) |
| docs | `docs/crates/render-mcp-cli.md`, `docs/crates/pipeline.md` |
| code review | no findings |

red: DBT-35, DBT-36: no heading "Sections marked wrong" in the report; no warning for an unreadable `reviews.json`.

Code review: no findings
not verified: none
decisions needed: none

## Deliverable 5
done: README; the fixes of the `high` review of the issue (8 findings): `retrodoc report` lists the sections marked
wrong from `debt::reviewed`, so it agrees with the debt view on the domains of the run; one constructor of the id of
a use case (`reviews::Section::use_case`); the validated sections fingerprinted once per request; the audit counts by
`HashSet`; the label of a reviewed kind chosen by its kind; the audit texts reworded so the French agrees (`Fonctionnalités
revues : 1 sur 4 · Validé : 1, …`) and a text of the audit page of its own for an unreadable `reviews.json`.

| check | result |
|---|---|
| tests | pass |
| lint | pass |
| specs | pass |
| docs | `README.md`, `docs/crates/render-mcp-cli.md` |
| code review | 2 findings |

red: none (fixes under the tests of deliverables 1 to 4; the AUD-16 and AUD-17 tests updated to the new wording).

Code review: 2 findings
- the docs loaded with no warning when absent: not changed, `GeneratedDocs::load` is `None` only without features
  or use cases, which `retrodoc report` refuses before; a comment says so.
- `features` and `use_cases` now dead in `report::run`: not so, `build_report` and `add_doc_debt` still read them.
not verified: none
decisions needed: none
