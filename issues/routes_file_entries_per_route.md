# Keep a routes file from listing every explicit route as an entry point

# Goal
The entry points prompt asks for one entry per resource or namespace in a routing file, "not one per route: the
controllers list the individual actions". On the Lobsters routes file the model applies it to `resources` blocks
only and lists every explicit `get`/`post` line as an entry, which duplicates the controller actions. Find out
whether the duplicates reach the docs, and if they do, make the routes file give one entry per resource or
namespace.

# Findings (frozen inputs of `lobsters`, deepseek-chat, read on 2026-10-29 at a238861, no LLM call)
* `config/routes.rb` (11.6k characters, 3 parts) has the entrypoint role and is not a business file: the
  entry points pass reads it alone, with its own prompt, which holds the routing-file rule.
* It gives 39 entries: `stories` and `comments` (from `resources`) are collapsed, while `GET /`, `GET /rss`,
  `GET /login`, `POST /login`, `GET /stories/url/all`… are one entry each. At least `GET /stories/url/all` and
  `GET /stories/url/latest` are also given by `app/controllers/story_urls_controller.rb`.
* In linkding the route declarations are in `bookmarks/urls.py`, which no role rule of the frozen roles matches
  (local qwen roles): no pass reads it.
* Unknown: whether the duplicates change the domains, features or use cases (the surface may merge them).

# Approach
1. Measure first, on the saved lobsters run: count the entries of `config/routes.rb` also given by a controller,
   and look for them in the surface, features and use cases.
2. If they matter, candidates (can be combined): make the rule explicit for explicit route lines (group them by
   controller or path prefix); drop from a routing file the entries a controller of the same run also gives.
3. Replay the entry points on frozen `lobsters` and `linkding` and compare, with the entries of the routing files
   removed from the cache first: the per-file hash ignores the prompt, so a replay would keep the old entries.

# Resources
* `crates/retrodoc-pipeline/src/entry_points/mod.rs` (`ENTRY_POINTS_SYSTEM_PROMPT`)
* `issues/done/joint_reading_routing_rule.md` (the measure that found it)
* The measures ledger (see `CLAUDE.md`), skill `quality-benchmark`

# Hints
* The per-file hash of the reading passes covers the content, the brief and the model, not the prompt
  (`batched_read.rs`): a prompt change re-reads nothing in a saved run.
* The linkding `bookmarks/api/routes.py` holds DRF view sets (controller actions): one entry per action is right
  there, so a rule keyed on the file name would be wrong.

# Tracking

Plan: validated

## Understanding
The duplicates reach the docs, whatever the shape of the routes entries: the later passes must keep one copy of a
route, the one of the controller. Out of scope: the prompt, the reading of `bookmarks/urls.py`. Means: a mechanical
filter of the entries before the features (like `without_unreached_public_api`), not a prompt change.

## Measure (no LLM call, saved runs, prototype of the filter in Python)
| run | routes.rb entries | same name in a controller | left out by the filter | use cases citing only left-out entries |
|---|---|---|---|---|
| lobsters `files` d8d8c92-1 (= frozen) | 39, one per route | 26 | 29 | 23 use cases of a feature `routing` (domain infrastructure) |
| lobsters `surface` 7e2bf9a (default path) | 53, grouped as the prompt asks | 6 by name, most by path prefix | 35 | 16 of 208 (feature `invitation-routing`, 6 copies of the invitations use cases, `signup`, mod mail, hats...) |
| linkding frozen, `rules-ld-judged` | `api/routes.py` 16-26 | 0 | 0 (no routing file) | 0 |
| delivery_router frozen | none | - | 0 | 0 |
| large private Rails app | 2 routing files of 3 | - | 32 | not measured |

* Grouping per resource does not remove the duplicates: the `surface` run follows the rule and still gets them.
* In d8d8c92-1 all 39 entries come from part 1 of 3 (last one at line 126, character 4,721): parts 2 and 3 gave
  nothing, with no warning. Not this issue.

## Plan
1. [x] Filter: `EntryPoints::without_routes_declared_elsewhere` applied by `generate` next to the public API filter
   (`application_entry_points`), info log of the ids; spec `specs/routing-file-duplicates.md`; verified by unit tests on
   built inventories (lobsters and linkding shapes) and an example `routing_duplicates_measure` on the saved runs above;
   makes true RFD-1 to RFD-8. ~400 lines.
2. [x] Measure: replay of features and use cases on frozen `lobsters` (paid, see question 2), compared with the
   d8d8c92 baselines (features judged recall 87%, use cases 202); ledger line; verified by the table in the issue.

## Decisions
- Mode: autonomous
- Base commit: 94a83b9
- Means: the mechanical filter, not a prompt change (question 1 a).
- Measure: replay features and use cases on frozen `lobsters` after deliverable 1, ~$0.3 (question 2 a).
- The empty parts 2 and 3 of `config/routes.rb` get an issue of their own (question 3 a).

## Deliverable 1 summary
done: `EntryPoints::without_routes_declared_elsewhere` (`entry_points/routing.rs`) applied by `generate` after the
public API filter, info log of the ids; example `routing_duplicates_measure`; spec RFD accepted (RFD-6 and RFD-8
reworded: one "must" per statement, and the ids returned rather than a log no test can read).

| check | result |
|---|---|
| tests | `just check` green (544 pipeline tests, 8 new) |
| lint | clippy pedantic clean |
| specs | RFD-1 to RFD-8 traced (internal: `generate` needs an LLM) |
| docs | `docs/crates/pipeline.md`, `docs/dev/commands.md`, module doc of `entry_points` |
| code review | `low`: no findings |

red: RFD-1, RFD-2, RFD-3, RFD-4, RFD-5, RFD-6, RFD-7, RFD-8: the stub left everything in (`left: []` against the 3
ids expected, `left: 0 right: 6`...), `paths_of` gave no path.

Measure (`routing_duplicates_measure`, saved runs, no LLM):

| run | entries left out | features holding only them | use cases citing only them |
|---|---|---|---|
| lobsters `surface` 7e2bf9a | 35 of 302 | 1 of 73 (`invitation-routing`) | 16 of 208 |
| lobsters `files` d8d8c92-1 | 29 of 284 | 0 of 98 | 15 of 392 |
| linkding `rules-ld-judged` | 0 of 80 | 0 | 0 |
| delivery_router `rules-dr-judged` | 0 of 5 | 0 | 0 |
| large private Rails app | 32 of 852 | 1 of 85 | 34 of 762 |

Code review: no findings

not verified: the effect on a generate (deliverable 2).
decisions needed: none.

## Deliverable 2 summary
done: the replays work from the entry points `generate` keeps (`prepare` and `prepare_with_tree` apply
`without_routes_declared_elsewhere`; they did not, so a replay measured the old inputs), then the paid measure on a
copy of the frozen inputs (`frozen/lobsters-rfd`, `frozen/lobsters` untouched), deepseek-chat, at a07df3c + this
deliverable.

| replay (lobsters, frozen) | items | judged recall | judged precision | calls | prompt tokens |
|---|---|---|---|---|---|
| features, without the filter, 3 runs | 79 (76-82) | 99% (97-100) | 47% (44-49) | 10 | 36.8k |
| features, with the filter, 3 runs | 80 (78-82) | 99% (97-100) | 44% (41-46) | 9 | 32.1k |
| use cases, with the filter, 1 run | 204 | business 81%, score=1 77% | | 82 | 900k |
| use cases d8d8c92 baseline (other frame, not comparable) | 202 | business 80%, score=1 72% | | 78 | 810k |

* On the frozen inputs the filter changes nothing measurable but the cost of the features pass (-13% prompt tokens):
  there the 39 routes entries were placed in the features of their controllers (14 features, 34 use cases cite
  one), so a use case already covered both copies. The 16 duplicate use cases are on the `surface` run, where
  grouped entries made features of their own (`invitation-routing`): only a new generate on that path measures them.
* A routes entry with no path (`stories`, from `resources :stories`, in the part-1 inputs) stays (RFD-6) and made a
  feature of its own in one run (`story-management-routes`).

| check | result |
|---|---|
| tests | `just check` green (1 new: `a_replay_works_from_the_entry_points_generate_keeps`) |
| lint | clippy pedantic clean |
| specs | n/a (benchmark tooling) |
| docs | `docs/crates/pipeline.md` (`benchmark/` row) |
| code review | `low`: no findings |

red: none (benchmark tooling, no spec id); the new test failed before the change with `left: ["config/routes.rb",
"invitations.rb", "signup.rb", "stories.rb"]`.

Code review: no findings

not verified: the effect on a `surface` generate (the 16 duplicate use cases).
decisions needed: whether to pay a lobsters `surface` generate (~$0.3) to verify it.
