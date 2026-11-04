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
