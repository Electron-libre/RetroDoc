---
priority: low
depends_on: [web_ui_feature_and_use_case_pages]
why: 'cross views over what the feature and use case pages already show: a comfort'
---
# Show a catalogue of the business rules and a matrix of the actors in the web interface

# Goal

Two cross views of the web interface: a catalogue of every business rule, filterable by kind (validation, permission,
state transition, limit, computation), by domain and by feature, each with its evidence and a link to its feature; and
a matrix of the actors by use case (who starts or takes part in what), with the permission rules beside each actor.

# Approach

1. A rules page with the filters as query parameters, re-rendered by `htmx`.
2. An actors page: the actors of the steps and the primary actors, human and system apart, a row per actor and a
   column per feature, a cell linking to its use cases.
3. Router tests on both pages and on a filter.

# Specs

* `specs/web-ui.md` amended.

# Milestone

Browse the docs in a web interface

# Resources

* `crates/retrodoc-core/src/model.rs` (`BusinessRule`, `RuleKind`, `RuleEvidence`, `Actor`)
* `crates/retrodoc-render/src/rules.rs`
* `docs/adr/0021-business-rules-as-a-model-object.md`

# Hints

* The actor names come from the LLM per use case: the same actor may be spelled two ways; group by the exact name
  and leave the merging out of scope.
* The page of a cited file (`crates/retrodoc-web/src/code.rs`, `citers`) links a rule to the top of its feature page:
  once the rules have a page or an anchor (`id` on their `<li>` in `macros.html`), link there.

# Tracking

Plan: validated

## Understanding
Two cross pages of `retrodoc ui`: `/rules`, every rule filterable by kind, domain and feature (query parameters,
`htmx` swaps the list), and `/actors`, a matrix actor × feature linking the use cases, with the permission rules of
each actor. Out of scope: merging the spellings of an actor. Also: the rule citers of a file page link to `#rule-<slug>`.

## Plan
1. [x] Rules page: `/rules` with its filter form and `htmx` fragment, link from every page, rule citers of a file page
   at `#rule-<slug>`; verified by router tests in `tests/rules_page.rs` and `tests/code_page.rs`; makes true WEB-52
   (rules link), WEB-53, WEB-54, WEB-55, WEB-56, WEB-61.
2. [x] Actors page: `/actors`, the matrix human then system, cells linking the use cases, permission rules per row,
   link from every page; verified by router tests in `tests/actors_page.rs`; makes true WEB-52, WEB-57 to WEB-60.
3. [x] Fixes of the `high` review of the issue: htmx history restore gets the full page (rules and search), feature
   choices labelled with their domain, a run with no rule says so, one `rule_href`, one fragment-or-page helper, the
   actors page built once; verified by router tests; makes true WEB-43, WEB-53, WEB-55, WEB-56 as amended.

## Decisions
- Mode: autonomous
- A primary actor no step names is a human actor (not read from `actors.yaml`).
- The matrix has a column only per feature where an actor appears (WEB-58).
- Base commit: 97254fea2a8bb13cb6e1a7320d55f18e39aae102

## Deliverable 1: rules page
- done: `/rules` (`rules.rs`, `rules.html`, `rules_list.html`) with its kind, domain and feature filters and its
  `htmx` fragment, a `Rules` link in the header, the rule citers of a file page at `#rule-<slug>`; evidence macro shared.
- red: WEB-52, WEB-53, WEB-54, WEB-55, WEB-56, WEB-61: `/rules` answered 404 (`assertion left == right failed: 404 vs 200`),
  no `href="/rules"` in the header, the rule citer linked to the top of its feature page.
- Code review: no findings
- not verified: the `htmx` swap in a real browser.

## Deliverable 2: actors page
- done: `/actors` (`actors.rs`, `actors.html`), the matrix human then system with the use cases of each cell and the
  permission rules of each row, an `Actors` link in the header; `specs/web-ui.md` accepted.
- red: WEB-52, WEB-57, WEB-58, WEB-59, WEB-60: `/actors` answered 404 (`no section human-actors in` the not-found page),
  no `href="/actors"` in the header.
- Code review: 1 finding
  - fixed: a primary actor whose name the steps give both kinds was marked `starts` (and its permission rules
    listed) in both rows; it now starts its use cases in the human row only (scenario added to WEB-57, test red then green).
- not verified: the layout of a wide matrix in a real browser.

## Deliverable 3: fixes of the high review
- done: `pages::fragment_or_page` shared by `/rules` and `/search`, a history restore (`HX-History-Restore-Request`)
  answered with the full page; the feature choices read `<feature> (<domain>)`; "No business rule was found" when the
  run has none; `view::rule_href` replaces four copies; `kind_value` is an exhaustive match; `/actors` built in
  `Site::new`. WEB-43, WEB-55, WEB-56 amended, a WEB-53 scenario added.
- red: WEB-43, WEB-53, WEB-55, WEB-56: the history restore got the fragment (no `<html`), no `(Shipping)` in the feature
  choice, "No rule matches." for a run with no rule.
- Code review: no findings (low review of this deliverable)
- high review findings: 8; 7 fixed as above; not fixed: the linear lookups of a rule's feature and domain
  (`rules.rs`), a few hundred string comparisons per request on a run, not worth a map.
