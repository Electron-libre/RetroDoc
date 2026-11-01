---
priority: high
depends_on: [web_ui_server_and_domains]
why: 'the body of the docs: without these pages the interface shows only the list of domains'
---
# Show the domain, feature and use case pages in the web interface

# Goal

Render in the web interface what the Markdown pages of `docs/functional/` hold, in a structured form: a domain page
with its features (the back-office ones set apart), a feature page with its description, entry points, rules and use
cases (the technical ones under "Administration and technical"), and a use case page with its narrative, primary
actor, steps (actor, action, detail, cited lines), the Mermaid diagram drawn in the browser, the business language and
confidence scores with their rationale, and the links to the existing docs that document or contradict it.

# Approach

1. One template per page, from the read model of `web_ui_server_and_domains`; a breadcrumb domain, feature, use case.
2. The vendored `mermaid` script, embedded like `htmx`, draws `diagram_mermaid`; the source of the diagram stays
   readable when the script fails.
3. The badges: confidence (flagged below `LOW_CONFIDENCE_THRESHOLD`), stale when a cited file changed since the run
   (`Freshness`), set apart (`UseCase::is_technical`, `Feature::back_office`), unbacked narrative (`narrative_check`).
4. The existing docs of the reconciliation (`DocLinks`) as links, `states otherwise` shown for a contradiction.
5. Router tests per page, an unknown slug answered by the 404 page.

# Specs

* `specs/web-ui.md`: the requirements of the three pages, amended at `issue-workflow` step 1.

# Milestone

Browse the docs in a web interface

# Resources

* `crates/retrodoc-render/src/markdown/` (`domain.rs`, `feature.rs`, `use_case.rs`): what each page shows today
* `crates/retrodoc-render/src/doc_links.rs`, `specs/docs-reconciliation.md`
* `crates/retrodoc-core/src/model.rs` (`UseCase::is_technical`, `Step::label`, the thresholds)
* `issues/done/technical_and_back_office_use_cases.md`

# Hints

* Show the same things as the Markdown pages, in the same order of sections, so the two never disagree; reuse the
  helpers of `retrodoc-render` rather than copying their rules.
* The `mermaid` script is large: serve it with a long cache header and load it only on the pages that draw a diagram.
