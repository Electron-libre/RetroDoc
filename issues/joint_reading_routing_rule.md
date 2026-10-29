# Give the joint reading the routing-file rule of the entry points prompt

# Goal
The joint reading (`business_read`, on by default since ADR 0028) asks for the entities and the entry points in
one prompt, written by hand from the two single-purpose prompts. It lacks one rule of the entry points prompt: a
routing file gives one entry per resource or namespace, not one per route. A routing file read jointly can then
come back with one entry per route. Bring the rule back, and keep the prompts from drifting again.

# Findings (exploration of 2026-10-29, at aee4cc9; read in the code, not measured)
* `crates/retrodoc-pipeline/src/business_read/mod.rs`: the comment above `BUSINESS_READ_SYSTEM_PROMPT` says the
  wording is that of `glossary` and `entry_points`, "keep them close"; the routing-file sentence of
  `ENTRY_POINTS_SYSTEM_PROMPT` (`entry_points/mod.rs`) is missing.
* The effect depends on whether a routing file is among the business files: those outside the list (entry point
  role) are read by the entry points pass alone, with its own prompt. Not measured.

# Approach
1. Measure first: on the frozen inputs of `lobsters` and `linkding`, is a routing file read by the joint reading,
   and how many entries does it give? (Ledger first: reuse a saved run if one has the business read.)
2. If it is: add the rule to the joint prompt, or build the joint prompt from parts shared with the two prompts so
   a change of one reaches the other.
3. Measure the entry points again.

# Resources
* ADR 0028; `crates/retrodoc-pipeline/src/business_read/`, `entry_points/`, `glossary/`
* The measures ledger (see `CLAUDE.md`), skill `quality-benchmark`

# Hints
* The per-file hash of the reading passes covers the content, the brief and the model; check whether it covers
  the prompt, else a saved run keeps the old entries until the file changes.

# Tracking

Plan: validated

## Understanding
Step 1 of the approach answered from saved data (no LLM call): on the frozen inputs of the two benchmark apps, no
routing file in the sense of the rule (route declarations) is read jointly, so the missing rule has no measurable
effect there. Proposed: record the measure and close without a prompt change.

## Measure (2026-10-29, at a238861, frozen inputs, no LLM call)
| repo | routing file | role | business file? | read by | entries |
|---|---|---|---|---|---|
| lobsters | `config/routes.rb` (11.6k chars, 3 parts) | entrypoint | no | entry points pass alone (its prompt has the rule) | 39: `resources` collapsed (`stories`, `comments`), every explicit `get`/`post` line one entry |
| linkding | `bookmarks/api/routes.py` | entrypoint | yes | joint reading | 16, one per action: DRF view sets with their `@action`s plus 5 router registrations, so controller actions, where one entry per action is what the rule asks |
| linkding | `bookmarks/urls.py` | no rule matches | no | not read | - |

* The per-file hash of the reading passes is `hash_for_model(brief.hash_with(content), llm)`
  (`batched_read.rs:140`): content, brief, model, not the prompt. A prompt change re-reads nothing in a saved run.
* Adding the rule to the joint prompt could collapse the linkding view sets (a file named `routes.py`) to one
  entry per resource: a risk with no gain measured.

## Plan
1. [x] Record the measure: this table in the issue, one ledger line; no code change; verified by `just check`.

## Decisions
- Mode: autonomous
- Base commit: a238861
- No prompt change: the rule stays out of the joint prompt and the prompts are not shared (risk on view sets, no gain measured) (1 a).
- The per-route entries of lobsters `config/routes.rb` go to a new issue (2 a).

## Deliverable 1
done: measure recorded (issue table, ledger line); follow-up issue `routes_file_entries_per_route` opened, ranked 2nd.

| check | result |
|---|---|
| tests | `just check` passes |
| lint | `just check` passes |
| specs | no capability changed |
| docs | none touched |
| code review | low, on the issue files |

red: none
Code review: no findings (low; issue files only, the new issue file untracked so unread)
not verified: whether the linkding frozen entry points came from the joint reading (inferred: business_read on by default since 2026-10-12, frozen 2026-10-23)
decisions needed: none

Done: no prompt change (decision 1 a); nothing left.
