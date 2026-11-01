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
