---
id: EDM
status: draft
adrs: [0020]
---

# Domains with no feature

## Intent

The domains pass names the domains before the features pass groups the entry points into features, so a domain
can end up holding no feature: the model named it for entities alone, or every feature of it was set aside as
technical. A domain page with no feature is the first thing a reader meets and tells them nothing; the docs leave
such a domain out and the debt report says it was left out.

## Requirements

- **EDM-1** (deterministic) A domain with no feature (no feature of `features.yaml` belongs to it) must not get a
  page under `functional/`.
- **EDM-2** (deterministic) A domain with no feature must not be listed in the agent index (`llms.txt`).
- **EDM-3** (deterministic) A domain with no feature must not be listed by the read model of the MCP server and the
  web interface.
- **EDM-4** (deterministic) A domain with no feature must not be an item of the reconciliation with the project's
  own docs.
- **EDM-5** (deterministic) The debt report must list each domain with no feature, with its name and the number of
  its features set aside as technical.
- **EDM-6** (deterministic) A domain with no feature must not be a row of the domain table of the debt report.

## Scenarios

- (EDM-1, EDM-2) Given the domains `orders` and `feeds`, and features in `orders` only, when the docs are rendered,
  then `functional/orders/README.md` is written, no `functional/feeds/` exists and `llms.txt` does not name `feeds`.
- (EDM-5) Given the domain `operations` whose two features were both set aside as technical, when the debt report is
  written, then it lists `operations` among the domains with no feature, with 2 features set aside.
- (EDM-1) Given a rerun after the last feature of `feeds` was set aside, when the docs are rendered, then the page
  of `feeds` written by the earlier run is removed (as any page the docs no longer hold, RND).

## Non-goals

- Keeping the domains pass from naming a domain with no resource: the domains pass is unchanged, the docs drop
  what it names that no feature fills.
- Removing the domain from `domains.yaml`: the artifact of the domains pass stays as the pass wrote it.
- The "uncategorized" bucket: it is already left out of the docs.
