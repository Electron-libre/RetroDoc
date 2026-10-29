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
* Unknown: whether the duplicates change the domains, features or use cases (the surface may merge them).

# Approach
1. Measure first, on the saved lobsters run: count the entries of `config/routes.rb` also given by a controller,
   and look for them in the surface, features and use cases.
2. If they matter, candidates (can be combined): make the rule explicit for explicit route lines (group them by
   controller or path prefix); drop from a routing file the entries a controller of the same run also gives.
3. Replay the entry points on frozen `lobsters` and `linkding` and compare.

# Resources
* `crates/retrodoc-pipeline/src/entry_points/mod.rs` (`ENTRY_POINTS_SYSTEM_PROMPT`)
* `issues/done/joint_reading_routing_rule.md` (the measure that found it)
* The measures ledger (see `CLAUDE.md`), skill `quality-benchmark`

# Hints
* The per-file hash of the reading passes covers the content, the brief and the model, not the prompt
  (`batched_read.rs`): a prompt change re-reads nothing in a saved run.
* The linkding `bookmarks/api/routes.py` holds DRF view sets (controller actions): one entry per action is right
  there, so a rule keyed on the file name would be wrong.
