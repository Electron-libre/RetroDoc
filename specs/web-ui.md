---
id: WEB
status: draft
adrs: [0039]
---

# Web interface

## Intent

`retrodoc ui` serves the docs of the last `generate` run to a human in a browser, so whoever takes an application
over browses its domains, features and use cases rather than reading the Markdown pages one at a time.

## Requirements

- **WEB-1** (deterministic) Without `--bind`, `retrodoc ui` must listen on `127.0.0.1:8730`.
- **WEB-2** (deterministic) With `--bind <addr>`, `retrodoc ui` must listen on that address.
- **WEB-3** (deterministic) `retrodoc ui` must print on stdout, once it listens, the URL of the home page.
- **WEB-4** (deterministic) The home page must list every domain of the last run with its name, its description, its
  confidence and its numbers of features and use cases.
- **WEB-5** (deterministic) Every page must show the tree of the domains, their features and the use cases of each
  feature, each item a link to its page.
- **WEB-6** (deterministic) A confidence below `LOW_CONFIDENCE_THRESHOLD` must be shown flagged as low.
- **WEB-7** (deterministic) A domain one of whose features cites a file modified or deleted since the run must be
  shown flagged as stale.
- **WEB-8** (deterministic) A path the interface does not serve must be answered with the status 404 and the
  not-found page.
- **WEB-9** (deterministic) On a repository initialized but with no run yet, the home page must answer with the status
  200 and say to run `retrodoc generate`.
- **WEB-10** (deterministic) Every text that comes from the docs must be HTML-escaped in the pages.
- **WEB-11** (deterministic) The pages must load their scripts and stylesheet from the interface itself, under
  `/assets/`, and from no other host.
- **WEB-12** (deterministic) Serving the pages must not write any file in the repository.

## Scenarios

- (WEB-2) Given `--bind 127.0.0.1:0`, then the URL printed gives the port the system picked.
- (WEB-4) Given a domain with no feature, then it is listed with zero features and zero use cases.
- (WEB-4) Given a domain with no confidence score yet, then it is listed without a confidence, not as low.
- (WEB-7) Given a cited file with no recorded hash (left out by the file budget), then the domain is not flagged.
- (WEB-8) Given the link of a feature in the tree, while the feature pages are not served yet, then it answers 404,
  not an error.
- (WEB-9) Given a repository where `retrodoc init` was never run, then the command fails like the other commands and
  asks to run `retrodoc init`.
- (WEB-10) Given a domain named `<script>alert(1)</script>`, then the page shows the name as text.

## Non-goals

- The domain, feature and use case pages, the search, the glossary and the cited code (later issues of the milestone).
- No authentication nor several repositories: the server is for the local user (a hosted mode needs its own ADR).
- No LLM call and no edit of the docs.
- The docs are read once at start: the pages of a `generate` run while the server runs show after a restart of `ui`.
