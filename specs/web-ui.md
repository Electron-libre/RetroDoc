---
id: WEB
status: accepted
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
- **WEB-11** (deterministic) The pages must load their scripts, stylesheet and images from the interface itself,
  under `/assets/`, and from no other host.
- **WEB-12** (deterministic) Serving the pages must not write any file in the repository.
- **WEB-13** (deterministic) A request whose `Host` names neither `localhost` nor an IP address must be refused with the
  status 403.
- **WEB-14** (deterministic) With a `--bind` address that is not a loopback one, `retrodoc ui` must warn that the docs
  are reachable from other machines.
- **WEB-15** (deterministic) The page of a domain, `/domains/<domain>`, must show in this order its name, its
  description, its confidence, its features each with its confidence and description (the back-office ones under
  "Administration and technical"), the business rules of its features each with its feature, and the existing docs
  that document or contradict it.
- **WEB-16** (deterministic) The page of a feature, `/domains/<domain>/<feature>`, must show in this order its name, its
  description, its confidence, its use cases each with its confidence and description (those `UseCase::is_technical`
  sets apart under "Administration and technical"), its business rules each with its kind, confidence, entities, the
  use cases it applies to and its evidence, the existing docs that document or contradict it, its source files and
  its entry points.
- **WEB-17** (deterministic) The page of a use case, `/domains/<domain>/<feature>/<use case>`, must show in this order
  its name, its description, its confidence, its business-language score, its primary actor, the entry points that
  trigger it, its narrative, the business rules that apply to it, its steps each with its actor and whether it is
  human or a system, its action, its detail and its cited lines (`path:start-end`), its diagram and the existing docs
  that document or contradict it.
- **WEB-18** (deterministic) Every score shown on a page of a domain, a feature or a use case must be shown with its
  rationale when it has one.
- **WEB-19** (deterministic) The pages of a domain, a feature and a use case must show a breadcrumb linking to the
  home page and to each level above the page.
- **WEB-20** (deterministic) The page of a feature or a use case must be flagged as stale, with the files that
  changed, when a file its source files or its cited lines name was modified or deleted since the run.
- **WEB-21** (deterministic) A back-office feature must be flagged as such on its page.
- **WEB-22** (deterministic) The narrative of a use case must be flagged as unbacked, with the rationale, when the
  confidence pass judged it unsupported by the cited code (`narrative_check` at 0).
- **WEB-23** (deterministic) Each existing doc section that documents or contradicts an item must be shown as a link
  to `/docs/<path>#<anchor of its heading>` labelled with its path and heading, and a contradiction with what the doc
  states otherwise.
- **WEB-24** (deterministic) A page with a diagram must load the `mermaid` script from `/assets/` to draw it and keep
  the source of the diagram in the page; a page without a diagram must not load it.
- **WEB-25** (deterministic) The status 404 and the not-found page must answer a domain, a feature or a use case
  slug the last run does not have, or a feature asked under a domain it does not belong to; a domain slug of a map
  (`/map/<domain>`) too.
- **WEB-26** (deterministic) The page of a file the docs cite, `/code/<path>`, must show its lines numbered, within the
  window `read_source` serves: from the first line, 200 lines by default and 400 at most, a line longer than 400
  characters cut.
- **WEB-27** (deterministic) With `?lines=<start>-<end>` (or `?lines=<start>`), the page of a file must show its lines
  from 10 before `start` to 10 after `end`, within the window of WEB-26, and mark the lines `start` to `end` as cited.
- **WEB-28** (deterministic) The page of a file must list the last 10 commits that changed it, each with its short
  hash, its date and its subject, without its author.
- **WEB-29** (deterministic) The page of a file must list the features, the use cases and the business rules that cite
  it, each a link to its page (a rule to the page of its feature).
- **WEB-30** (deterministic) The page of a file must be flagged as stale when the file was modified or deleted since
  the run.
- **WEB-31** (deterministic) The status 404 and the not-found page must answer a `/code/` path that is not a plain
  path inside the repository, or that no feature, step or rule cites.
- **WEB-32** (deterministic) The page of a cited file must say why and show none of its lines when `read_source`
  refuses to show it (a symbolic link on its way, deleted, binary, larger than 1 MB, a range past its end).
- **WEB-33** (deterministic) Each cited line of a step, source file of a feature and code evidence of a rule must be a
  link to the page of its file, at its lines when it has some.
- **WEB-34** (deterministic) The files page, `/files`, must show the tree of the source files of the last run (those
  whose hash it recorded, and those the docs cite), each directory with the number of its files cited out of its
  files, each file with the number of features, use cases and rules that cite it.
- **WEB-35** (deterministic) On the files page, a cited file must be a link to its page.
- **WEB-36** (deterministic) On the files page, a file no doc cites must be flagged `not cited`, with no link.
- **WEB-37** (deterministic) Every page must link to the files page.
- **WEB-38** (deterministic) The page of a file whose window ends before its last line must link to the page showing
  the next lines, `?from=<line>`, which shows the window from that line with no line marked.
- **WEB-39** (deterministic) Every page must show a search box that sends its query to `/search?q=<query>`.
- **WEB-40** (deterministic) The search page must show the hits the index of `search_docs` returns for the query, by
  the same relevance rule (DSR-1, DSR-2), at most 10, grouped by kind in the order domain, feature, use case, rule,
  concept, collected doc, best first within a kind.
- **WEB-41** (deterministic) Each hit must show its title, its confidence when it has one, and be a link to its page:
  a domain, a feature or a use case to its page, a rule to its item on the page of its feature (`#rule-<slug>`), a concept to
  `/glossary#<anchor of its name>`, a collected doc to `/docs/<path>`.
- **WEB-42** (deterministic) With no hit, the search page must say the topic is not documented, give the number of
  entries DSR-2 left out when there are any, and name the words of the query no entry has (DSR-4, DSR-5).
- **WEB-43** (deterministic) A search request with the header `HX-Request` and without `HX-History-Restore-Request`
  must be answered with the results alone, without the layout of the page; any other with the full page.
- **WEB-44** (deterministic) Every page must link to the glossary page.
- **WEB-45** (deterministic) The glossary page, `/glossary`, must list every concept of the glossary of the last run,
  sorted by name ignoring case, each with its definition, under a heading whose anchor is `anchor` of its name.
- **WEB-46** (deterministic) Each concept of the glossary page must list the use cases that use it, each a link to
  its page: a use case uses a concept when one of its steps cites a file the
  concept was read from, or when a business rule that applies to the use case names the concept.
- **WEB-47** (deterministic) The page of a collected doc, `/docs/<path>`, must show the Markdown doc of the
  repository at that path, among those `retrodoc generate` reads, rendered as HTML.
- **WEB-48** (deterministic) Each heading of a collected doc page must carry the anchor `anchor` gives its text, the
  one the links of WEB-23 point to.
- **WEB-49** (deterministic) The raw HTML of a collected doc must be shown as text, not as markup.
- **WEB-50** (deterministic) A link or an image of a collected doc must be shown as its text alone when its URL has a
  scheme other than `http`, `https` or `mailto`.
- **WEB-51** (deterministic) The status 404 and the not-found page must answer a `/docs/` path that is not one of the
  collected docs.
- **WEB-52** (deterministic) Every page must link to the rules page and to the actors page.
- **WEB-53** (deterministic) The rules page, `/rules`, must list every business rule of the last run, in the order of
  the run, each with its statement, its kind, its confidence, its evidence as on the page of its feature (WEB-33), and
  a link to its item on the page of its feature (`#rule-<slug>`) labelled with the feature and its domain.
- **WEB-54** (deterministic) The rules page must keep only the rules of the kind `?kind=<kind>` (`validation`,
  `permission`, `state_transition`, `limit`, `computation`), of the domain `?domain=<slug>` and of the feature
  `?feature=<slug>`, each given, all of them when several are, and say no rule matches when none does.
- **WEB-55** (deterministic) The rules page must show a form whose choices are the kinds, the domains and the
  features of the run (each feature labelled with its domain), the ones of the query selected, that asks the page
  again with them.
- **WEB-56** (deterministic) A rules request with the header `HX-Request` and without `HX-History-Restore-Request` must
  be answered with the list of the rules alone, without the layout of the page; any other with the full page.
- **WEB-57** (deterministic) The actors page, `/actors`, must show a row per actor, the actors grouped by their exact
  name, the human ones then the system ones under their own heading, each group sorted by name ignoring case; the
  actors are the primary actors of the use cases and the actors of their steps.
- **WEB-58** (deterministic) The actors page must show a column per feature with at least one use case that names an
  actor, in the order of the run, headed by a link to its page.
- **WEB-59** (deterministic) The cell of an actor and a feature must link to each use case of the feature the actor
  starts (its primary actor) or takes part in (the actor of one of its steps), marking those it starts; a cell with
  none is empty.
- **WEB-60** (deterministic) Each row of the actors page must list beside the actor the permission rules that apply
  to a use case of its row, each a link to its item on the page of its feature.
- **WEB-61** (deterministic) On the page of a cited file, a rule that cites it must link to its item on the page of
  its feature (`#rule-<slug>`).
- **WEB-62** (deterministic) The name of the application must be the `name` of the `[project]` section of
  `retrodoc.toml` when it is set and not blank, else the name of the directory of the repository.
- **WEB-63** (deterministic) Every page must show the name of the application in its header and end its title with
  ` · <name>`.
- **WEB-64** (deterministic) The home page must have the name of the application as its heading and, when the saved
  product brief (`.retrodoc/cache/product.yaml`) has a purpose, show that purpose under it.
- **WEB-65** (deterministic) The home page must show, in this order, the heading and purpose of WEB-64, the users of
  the saved product brief when it has some, a "Start here" section, then the domains of WEB-4.
- **WEB-66** (deterministic) The "Start here" section of the home page must link to the map, actors, glossary, rules
  and files pages, each with one line saying what the page holds and its count (domains, actors, concepts, rules,
  cited files of the run).
- **WEB-67** (deterministic) The header of every page must group its links under "Read" (the map and actors pages)
  and "Look up" (the glossary, rules and files pages), each link with a title saying what its page holds.
- **WEB-68** (deterministic) The page of a use case must link to the previous and the next use case of its feature, in
  the order of the run, each labelled with its name; the first has no previous link and the last no next one.
- **WEB-69** (deterministic) Every page must end with a credit line showing the Meta Lambda logo, served from
  `/assets/`, and the text "Made by Meta Lambda", a link to `https://meta-lambda.com`.
- **WEB-70** (deterministic) The map page, `/map`, must keep the source of a graph with a node per domain of the last
  run, labelled with its name and its numbers of features and use cases, and an edge between two domains that share
  a concept, labelled with the names of the concepts they share, the first 3 by name then the number of the others.
  A domain uses a concept when one of the use cases of its features uses it (WEB-46).
- **WEB-71** (deterministic) The map page must list each domain as a link to its map, `/map/<domain>`, with the
  concepts it shares with each other domain, each concept a link to `/glossary#<anchor of its name>`.
- **WEB-72** (deterministic) The map of a domain, `/map/<domain>`, must keep the source of a graph with a node per
  feature of the domain, labelled with its name and its number of use cases, a node per human actor of their use
  cases with an edge to each feature it starts a use case of (solid) or only takes part in (dotted), and a node per
  concept their use cases use with an edge to each feature that uses it.
- **WEB-73** (deterministic) The map of a domain must list each of its features as a link to its page, with its human
  actors, each a link to the actors page, and its concepts, each a link to `/glossary#<anchor of its name>`.
- **WEB-74** (deterministic) Each node of a map graph must have an id in its source that the link of its item in the
  list of the page carries as `data-node`, so the drawn node opens the page of that link.
- **WEB-75** (deterministic) Each label of a map graph must be written between double quotes, with a `#` of the docs
  written `#35;` and a double quote written `#quot;`.
- **WEB-76** (deterministic) The page of a domain must link to its map.

## Scenarios

- (WEB-2) Given `--bind 127.0.0.1:0`, then the URL printed gives the port the system picked.
- (WEB-4) Given a domain with no feature, then it is listed with zero features and zero use cases.
- (WEB-4) Given a domain with no confidence score yet, then it is listed without a confidence, not as low.
- (WEB-7) Given a cited file with no recorded hash (left out by the file budget), then the domain is not flagged.
- (WEB-8) withdrawn: the feature pages are served (WEB-16); an unknown slug is WEB-25.
- (WEB-9) Given a repository where `retrodoc init` was never run, then the command fails like the other commands and
  asks to run `retrodoc init`.
- (WEB-13) Given a page of another site that resolves its own name to `127.0.0.1` (DNS rebinding), then its requests
  carry its name in `Host` and are refused; `localhost:8730`, `127.0.0.1:8730` and `[::1]:8730` are served.
- (WEB-10) Given a domain named `<script>alert(1)</script>`, then the page shows the name as text.
- (WEB-10, WEB-24) Given a diagram whose source holds `</pre><script>`, then the source is escaped like any text.
- (WEB-16) Given a feature with no use case, then the page says none could be derived from the code.
- (WEB-17) Given a use case with no narrative (artifacts from before it existed), then the page shows the steps
  without a narrative section.
- (WEB-17) Given a step with no cited line, then it is shown without cited lines; a reference with only a start line
  shows `path:start`, one with no line shows the path.
- (WEB-18) Given a use case with no business-language score, then the page shows no business-language line.
- (WEB-20) Given a cited file with no recorded hash, then the page is not flagged.
- (WEB-22) Given a narrative judged partly backed (`narrative_check` above 0), then it is not flagged.
- (WEB-23) Given a doc section with no heading, then the link points to the file with no anchor.
- (WEB-23) Given a doc path with a `#`, a `?` or a space, then they are percent-encoded in the link.
- (WEB-24) Given a client whose `Accept-Encoding` does not name gzip, then the script, stored compressed, is refused
  with the status 406.
- (WEB-23, WEB-48) Given a link of WEB-23, then it opens the page of the collected doc at the heading of its section.
- (WEB-25) Given a feature slug under the slug of another domain, then it answers 404 rather than the feature.
- (WEB-27) Given `?lines=` that is not a line or a range of lines, then the page shows the file from its first line,
  with no line marked.
- (WEB-27) Given `?lines=1-3`, then the window starts at line 1.
- (WEB-32) Given `?lines=` whose start is past the end of a file whose last lines are within the 10 lines before it,
  then the page says the range is past the end rather than showing those last lines.
- (WEB-32) Given an empty cited file, then the page says it is empty; with `?lines=`, that the range is past the end.
- (WEB-38) Given cited lines wider than the window, then the link to the next lines follows the last line shown.
- (WEB-28) Given a repository with no git history, then the page says the history is not available and still shows
  the lines.
- (WEB-30) Given a file with no recorded hash, then the page is not flagged.
- (WEB-31) Given `/code/../secret.txt`, `/code/%2E%2E/secret.txt` or an uncited file of the repository, then the
  answer is 404 and holds none of its content.
- (WEB-32) Given a cited symbolic link to a file outside the repository, then the page says links are not followed
  and holds none of the target's content.
- (WEB-33) Given a cited reference with no line, then it links to the page of its file with no `lines`.
- (WEB-34) Given a cited file whose hash the run did not record (left out by the file budget), then it is in the tree.
- (WEB-34) Given a directory of two files of which one is cited, then it shows `1/2` cited.

- (WEB-40) Given an empty query, then the search page shows the box and no result, nor the not-documented answer.
- (WEB-40) Given more than 10 hits, then the 10 best are shown: the same as `retrodoc search` with its default limit.
- (WEB-41) Given a rule or a use case whose feature is not in the run (artifacts of an older run), then it is shown
  without a link.
- (WEB-10, WEB-42) Given a query `<script>`, then the words it names are shown as text.
- (WEB-45) Given a repository with no glossary saved, then the page says no concept was found.
- (WEB-46) Given a concept no use case uses, then it is listed with no use case.
- (WEB-47) Given a repository with no run yet, then its collected docs are still served.
- (WEB-47) Given a doc that opens on a YAML front matter, then the page shows the doc without it.
- (WEB-43) Given a repository with no run yet, then an `htmx` search gets a fragment that says to run `generate`.
- (WEB-48) Given two headings with the same text, then both carry the same anchor, as the links of WEB-23 do.
- (WEB-50) Given `[x](javascript:alert(1))`, then the page shows `x` with no link; a relative link is kept.
- (WEB-53) Given a rule whose feature is not in the run (artifacts of an older run), then it is listed without a link,
  and no `?domain=` keeps it.
- (WEB-53) Given a run with no business rule, then the page says none was found rather than that none matches.
- (WEB-54) Given a filter whose value names no kind, domain or feature of the run, then no rule matches.
- (WEB-54) Given an empty filter value (`?kind=`), then it filters nothing, as when it is absent.
- (WEB-57) Given `Clerk` and `clerk`, then they are two rows (no merging of the names).
- (WEB-57) Given a primary actor no step names, then it is a human actor; given a name the steps give both kinds,
  then it is a row under each, and as a primary actor it starts its use cases in the human row only.
- (WEB-57) Given a run with no use case that names an actor, then the page says no actor was found.
- (WEB-59) Given an actor that both starts a use case and takes part in one of its steps, then the use case is linked
  once, marked as started.
- (WEB-53, WEB-57) Given a repository with no run yet, then both pages say to run `retrodoc generate`.
- (WEB-62) Given `name = "  "` under `[project]`, then the name is that of the repository's directory.
- (WEB-63) Given a repository with no run yet, then the page that says to run `retrodoc generate` names the
  application too.
- (WEB-64) Given a run whose brief was not written (`product.yaml` missing), then the home page shows the name and the
  domains, with no purpose.
- (WEB-65) Given a brief with no user, then the home page shows no users line.
- (WEB-66) Given a run with no business rule, then the rules link is still shown, with the count 0.
- (WEB-68) Given a feature with a single use case, then its page has neither a previous nor a next link.
- (WEB-70) Given a run with a single domain, then the graph has its node and no edge.
- (WEB-70) Given a concept no use case uses, then it links no domain.
- (WEB-72) Given an actor that both starts a use case of a feature and takes part in another, then their edge is
  solid, once.
- (WEB-72) Given a step whose actor is a system, then it is no node of the map.
- (WEB-72) Given a primary actor the steps of the run name a system only, then it is no node of the map, as it is no
  human on the actors page; given a human step with a blank name, then it is no node either.
- (WEB-70, WEB-72) Given a repository with no run yet, then both pages say to run `retrodoc generate`.
- (WEB-75) Given a feature named `a"] --> x["b`, then its label stays one node: `a#quot;] --> x[#quot;b`; given
  `Ticket #12;`, then the label is `Ticket #35;12;`.

## Non-goals

- No synonym matching nor search-as-you-type: the search is that of `retrodoc search` (`specs/docs-search.md`).
- The glossary shows no attributes nor associations of a concept, and has no page per concept.
- The files and images a collected doc links to are not served: only the collected Markdown docs are.
- No line of a file the docs don't cite is ever served: the files page shows only the path of such a file (ADR 0016).
- No syntax highlighting: the code is plain text with line numbers.
- The diagram is drawn by the browser: whether `mermaid` succeeds on an LLM-written diagram is not required, only that
  its source stays in the page.
- The actors page does not merge the spellings of an actor nor maps them to the actors list (`actors.yaml`).
- The rules page has no free-text filter: the search covers the rules (WEB-40).
- No authentication nor several repositories: the server is for the local user (a hosted mode needs its own ADR).
- No LLM call and no edit of the docs.
- The name of the application is not asked of the LLM, nor read from the `origin` remote; the Markdown render and the
  MCP answers don't carry it.
- The home page leaves the debt view and the audit dashboard to their milestone ("Explore the documentation debt and
  audit it"): it links to neither until they exist. Nor does it show the external systems and the open
  questions of the brief: only its purpose and its users.
- The colours of the interface (the charter of the Meta Lambda site) are no requirement: they are checked by eye, not
  by a test.
- The docs are read once at start: the pages of a `generate` run while the server runs show after a restart of `ui`.
- The maps draw no use case: a domain of a large application has hundreds; they are counted on their feature. Nor do
  they draw use cases that call one another: the docs hold no such link.
- The layout of a map is that of `mermaid`, drawn by the browser: how it reads is checked by eye, not by a test.
