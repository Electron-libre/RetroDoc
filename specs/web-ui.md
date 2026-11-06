---
id: WEB
status: accepted
adrs: [0039, 0044, 0048]
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
- **WEB-12** (deterministic) Serving the pages must not write any file in the repository other than
  `.retrodoc/reviews.json`, which only a review sent from a page (WEB-81) writes, and `.retrodoc/edits.json`, which
  only an edit sent from a page (WEB-93) writes, and, with `--ask`, the usage history of its calls (WAK-23).
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
  path inside the repository, or that no feature, step or rule cites and no item of the debt page lists (DBT-21).
- **WEB-32** (deterministic) The page of a cited file must say why and show none of its lines when `read_source`
  refuses to show it (a symbolic link on its way, deleted, binary, larger than 1 MB, a range past its end).
- **WEB-33** (deterministic) Each cited line of a step, source file of a feature and code evidence of a rule must be a
  link to the page of its file, at its lines when it has some.
- **WEB-34** (deterministic) The files page, `/files`, must show the tree of the source files of the last run (those
  whose hash it recorded, those the docs cite and those the debt page lists), each directory with the number of its
  files cited out of its files, each file with the number of features, use cases and rules that cite it.
- **WEB-35** (deterministic) On the files page, a cited file must be a link to its page.
- **WEB-36** (deterministic) On the files page, a file no doc cites must be flagged `not cited`, with a link to its
  page only when an item of the debt page lists it (DBT-21).
- **WEB-37** (deterministic) Every page must link to the files page.
- **WEB-38** (deterministic) The page of a file whose window ends before its last line must link to the page showing
  the next lines, `?from=<line>`, which shows the window from that line with no line marked.
- **WEB-39** (deterministic) Every page must show a search box that sends its query to `/search?q=<query>` on
  submit, and as an `htmx` request while it is typed, 300 ms after the last change, the answer to a request still
  running replaced by that of the last one.
- **WEB-40** (deterministic) The search page must show the hits the index of `search_docs` returns for the query, by
  the same relevance rule (DSR-1, DSR-2), at most 10, grouped by kind in the order domain, feature, use case, rule,
  concept, collected doc, best first within a kind.
- **WEB-41** (deterministic) Each hit must show its title, its confidence when it has one, and be a link to its page:
  a domain, a feature or a use case to its page, a rule to its item on the page of its feature (`#rule-<slug>`), a concept to
  `/glossary#<anchor of its name>`, a collected doc to `/docs/<path>`.
- **WEB-42** (deterministic) With no hit, the search page must say the topic is not documented, give the number of
  entries DSR-2 left out when there are any, and name the words of the query no entry has (DSR-4, DSR-5).
- **WEB-43** (deterministic) A search request with the header `HX-Request` and without `HX-History-Restore-Request`
  must be answered with the hits panel of WEB-109 alone, without the layout of the page, empty when the query holds
  fewer than 2 characters once trimmed; any other with the full page, which shows the results of WEB-40 to WEB-42 in
  its content.
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
  features of the run (each feature labelled with its domain), the ones of the query selected, in the shape of
  WEB-107, that asks the page again with them.
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
- **WEB-66** (deterministic) The "Start here" section of the home page must link to the map, actors, glossary, rules,
  files and debt pages, each with one line saying what the page holds and its count (domains, actors, concepts, rules,
  cited files of the run, debt items).
- **WEB-67** (deterministic) The header of every page must group its links under "Read" (the map and actors pages),
  "Look up" (the glossary, rules and files pages) and "Audit" (the debt page, DBT-13, the dashboard, AUD-9, and the history page, HPG-15), each link with a title saying
  what its page holds.
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
- **WEB-77** (deterministic) With `[output] language` naming French (`fr` or `French`, in any case), the fixed text
  of every page (headings, labels, links, titles of the links, the placeholder of the search box, the messages of the
  empty and not-found pages, but not the credit line of WEB-69) must be the French one; with no language or any other
  language, the English one.
- **WEB-78** (deterministic) The `<html>` element of every page must carry `lang` set to the tag of `[output]
  language` in `LANGUAGE_TAGS`, found as DLG-9 says, when it has one, `en` when no language is set, and no `lang` when the language has no
  tag.

- **WEB-79** (deterministic) The page of a domain, a feature or a use case must show, after its confidence, the review
  of its section (REV-1): its status as a badge, its comment, its author and its date; with no review, that it is not
  reviewed.
- **WEB-80** (deterministic) Each row of a list of sections must show the status of its review as a badge beside its
  confidence, and no badge when it has no review: the domains of the home page, the features of the page of a domain,
  the use cases of the page of a feature.
- **WEB-81** (deterministic) The page of a domain, a feature or a use case must hold a form, sent with `htmx`, that
  sends by a `POST` to the URL of the page a status (`validated`, `wrong`, `to_review`, or none) and a comment; a
  status records the review of the section with the comment (REV-12), none removes its review (REV-13).
- **WEB-82** (deterministic) The reviews must be read from `.retrodoc/reviews.json` at each request, so that a page
  shows a review sent before it without a restart of `retrodoc ui`.
- **WEB-83** (deterministic) A review `POST` with the header `HX-Request` must be answered with the review of the
  section and its form alone, swapped in place of the former ones; any other with a redirect `303` to the page
  followed by `?saved=review`.
- **WEB-84** (deterministic) A `POST` must be refused with the status 403, and nothing saved, unless its `Origin`
  header, else with no `Origin` its `Referer`, is `http://` followed by the value of its `Host` header.
- **WEB-85** (deterministic) A review `POST` to a section the last run does not have, or before a first run, must be
  answered with the status 404, and nothing saved.
- **WEB-86** (deterministic) A review `POST` whose status is not one of the four of WEB-81 must be answered with the
  status 400, and nothing saved.
- **WEB-87** (deterministic) The page of a section must show, in place of its review, the error of a
  `.retrodoc/reviews.json` that cannot be read or is of a later format (REV-10).
- **WEB-88** (deterministic) A review `POST` must be answered with the status 500 and the error when
  `.retrodoc/reviews.json` cannot be read or is of a later format, the file left as it was (REV-11).
- **WEB-89** (deterministic) The review of a section on its page must be flagged outdated when it is (REV-20,
  REV-21), with its reasons: the section rewritten since the review, the files changed since the review.
- **WEB-90** (deterministic) The badge of a review in a list of sections (WEB-80) must say when the review is
  outdated.
- **WEB-91** (deterministic) The page of an edited domain, feature or use case (`specs/section-edit.md`) must say,
  before its review, that the section holds a human edit: the fields it changes, its author when it has one and its
  date; a section with no edit shows none.
- **WEB-92** (deterministic) The edit of a section on its page must be flagged outdated when it is (SED-34, SED-35),
  with its reasons as WEB-89 says, and show, beside each field the edit changes, the text the last run generated for
  it: folded under the edit when the edit is not outdated, unfolded when it is.
- **WEB-93** (deterministic) The page of a domain, a feature or a use case must hold a form, sent with `htmx`, that
  sends by a `POST` to `/edit` followed by the URL of the page (`/edit/domains/<domain>`, a path no page has) its name,
  its description and, for a use case, its narrative, each filled with the text the page shows (SED-23).
- **WEB-94** (deterministic) A field the edit form sends must be saved with its surrounding whitespace trimmed and
  the carriage return of each line end a browser sends (CR LF) dropped.
- **WEB-95** (deterministic) A field of the edit form must be left out of the edit (SED-2) when it is sent empty or
  equal, once trimmed (WEB-94), to the text the last run generated for it, so that a form that leaves every field out
  removes the edit of the section (SED-16).
- **WEB-96** (deterministic) The edit form of an edited section must hold a button that brings the generated text
  back: it removes the edit of the section (SED-16).
- **WEB-97** (deterministic) An edit `POST` saved with the header `HX-Request` must be answered with the header
  `HX-Redirect` naming the page followed by `?saved=edit`, so that the whole page shows the edit; any other with a
  redirect `303` to the same URL.
- **WEB-98** (deterministic) An edit `POST` to a section the last run does not have, or before a first run, must be
  answered with the status 404, and nothing saved; one that sends a narrative for a domain or a feature, with the
  status 400 (SED-3), and nothing saved.
- **WEB-99** (deterministic) An edit `POST` must be answered with the status 500 and the error when
  `.retrodoc/edits.json` cannot be read or is of a later format, the file left as it was (SED-14); with the header
  `HX-Request`, as the edit form of the section alone, the fields as sent, swapped in place of the former
  ones.
- **WEB-100** (deterministic) Each row of a list of sections (WEB-80) must show a mark when its section holds a human
  edit, and none when it holds none.
- **WEB-101** (deterministic) The header of every page must link to the ask page, `/ask` (`specs/web-ask.md`), when
  `retrodoc ui` was started with `--ask`, and only then.
- **WEB-102** (deterministic) The review form, the edit form and the ask form must each hold an indicator that says,
  in the language of the interface, that the form is being sent, shown while its request runs (`hx-indicator`), and
  disable their buttons while it runs (`hx-disabled-elt`); the search form of the header and the filters of the debt
  and rules pages likewise hold such an indicator, their controls kept enabled.
- **WEB-103** (deterministic) The answer of WEB-83 to a review saved must hold, in the form, a notice that says the
  review is saved (`role="status"`); the review form of a page asked without `?saved=review` holds none.
- **WEB-104** (deterministic) The page of a domain, a feature or a use case asked with `?saved=review` must show the
  notice of WEB-103 in its review form, and asked with `?saved=edit`, a notice in its edit form that says the edit is
  saved; another value of `saved`, or none, shows no notice, nor does a review form that shows instead the error of
  WEB-87.
- **WEB-105** (deterministic) A review or an edit `POST` with the header `HX-Request` that is refused (WEB-85,
  WEB-86, WEB-98) or fails (WEB-88, WEB-99) must be answered, with its status, as the form of the section alone, the
  fields as sent, the error inside the form above its buttons (`role="alert"`), and the header `HX-Reswap`, swapped
  in place of the former form; the review form shows above it the review of the section as it is saved, when the run
  has the section.
- **WEB-106** (deterministic) Every page must swap an error answer (4xx, 5xx) to an `htmx` request when it names
  its swap with the header `HX-Reswap` (WEB-105, WAK-40), and show any other error answer in a notice of the page
  that names its status, and its text when it is not HTML, leaving the form as it was; a request sent clears the
  errors of the form that sends it.
- **WEB-107** (deterministic) The filter forms of the debt (DBT-10) and rules (WEB-55) pages must offer the kinds as
  a group of radio choices (`fieldset`), the first "All" with an empty value, the kind of the query checked, "All"
  when the query gives none or an empty one; the domains and the features as selects whose first choice "All" has
  an empty value. The form asks the page again on any change (`hx-trigger="change"`, its URL pushed), and its submit
  button is inside `<noscript>`.
- **WEB-108** (deterministic) The list of the debt and rules pages, in the page and in the fragment of DBT-12 and
  WEB-56, must say how many filters the query gives a non-empty value (`1 active filter`, `2 active filters`) with a
  link that resets them, to the page with no query; with no active filter it shows neither.

- **WEB-109** (deterministic) The hits panel must list the hits of WEB-40 in its order, each with its title, the
  name of its kind, its confidence when it has one and the link of WEB-41, then a link to the search page of the
  query; with no hit, it says the topic is not documented, with the link to the search page.
- **WEB-110** (deterministic) The search box must be a combobox: `role="combobox"`, `aria-autocomplete="list"`,
  `aria-controls` naming the hits panel, `aria-expanded` true while the panel shows an answer; the panel a
  `role="listbox"` whose hits and link to the search page are each a `role="option"` with an `id` of its own.
- **WEB-111** (deterministic) On every page, the key `/` pressed outside a field (an input, a text area, a select,
  an editable element) must put the focus in the search box and select its text, and not be typed.
- **WEB-112** (deterministic) In the search box, the keys must drive the panel: the arrows Down and Up move its
  active option (`aria-activedescendant`, `aria-selected`), from the last to the first and back; Enter follows the
  link of the active option, and with none submits the box; Escape, and a click outside the box and the panel, close
  the panel.

- **WEB-113** (deterministic) The page of a domain, a feature or a use case must show, right under its heading, its
  actions: a primary button "Review" that links to the review form of the page (`#review`) and a secondary button
  "Edit" that links to the page with `?edit=1` at its edit form (`?edit=1#edit-form`).
- **WEB-114** (deterministic) The page of a domain, a feature or a use case asked with `?edit=1` must show its edit
  form unfolded; with another value of `edit`, or none, folded unless WEB-104 or an error unfolds it.
- **WEB-115** (deterministic) Each item of the debt page whose section is a domain, a feature or a use case of the
  run must offer the actions "Review" and "Edit", links to that section as WEB-113 gives them, the group of the two
  labelled with the name of the section; an item of a rule, a folder or a section the run does not have offers none.
- **WEB-116** (deterministic) The home page must show, under its heading and purpose, a primary button to the debt
  page labelled with the number of its items when the run has some, to the map page when it has none, and no button
  when the run has no domain (WEB-117 says what to do).
- **WEB-117** (deterministic) The home page and the map page of a run with no domain must say why (no source file
  was grouped into a domain) and what to run: `retrodoc scan` to see the source files the run reads, then
  `retrodoc generate`.
- **WEB-118** (deterministic) The page of a use case must show its narrative, and the narrative the last run
  generated for its edit (WEB-92), rendered from Markdown by the rules of a collected doc: its raw HTML shown as
  text (WEB-49), a link or an image whose URL has an unsafe scheme shown as its text alone (WEB-50).
- **WEB-119** (deterministic) The edit form of a use case must hold a preview of its narrative, filled with the
  narrative the form holds rendered as WEB-118, that the form asks again with `htmx` by a `POST` to `/preview`
  followed by the URL of the page of the use case (`/preview/domains/<d>/<f>/<u>`) of its field `narrative` when that field changes (`hx-trigger="input changed delay:500ms"`), the answer swapped into
  the preview; the form of a domain or of a feature holds none.
- **WEB-120** (deterministic) A `POST` to `/preview/domains/<d>/<f>/<u>` must be answered with the status 200 and
  the `narrative` it sends rendered as the narrative of that use case (WEB-118), without the layout of a page, empty
  when the narrative is empty or absent, with no file written (WEB-12); the status 404 when the last run has no such
  use case.
- **WEB-121** (deterministic) The button of the edit form that brings the generated text back (WEB-96) must ask for
  a confirmation before it is sent (its `data-confirm`, read by `forms.js`), in a text that says the edit is removed.
- **WEB-122** (deterministic) A heading of a narrative rendered as WEB-118 must carry no `id` and be shown two
  levels below its Markdown level, under the `h2` of its section: `#` as an `h3`, `##` as an `h4`, any deeper level
  as an `h6`.
- **WEB-123** (deterministic) A relative link or image of a narrative rendered as WEB-118 must be resolved against
  the path of the Markdown page of its use case (`<docs dir>/functional/<domain>/use-cases/<feature>/<use case>.md`
  from the repository root): to `/docs/<path>`, its fragment kept, when a collected doc is at the resolved path;
  else shown as its text alone. A link of a fragment alone is kept as written.

## Scenarios

- (WEB-2) Given `--bind 127.0.0.1:0`, then the URL printed gives the port the system picked.
- (WEB-4) Given a domain with no feature, then it is not listed (EDM-3).
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
- (WEB-78) Given `language = "German"`, then the pages carry `lang="de"` and the fixed text stays English.
- (WEB-78) Given `language = "Japanese"`, then the pages carry no `lang`.
- (WEB-78) Given `language = "Français"` or `"fr-FR"`, then the pages carry `lang="fr"`.
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
- (WEB-31) Given `/code/../secret.txt`, `/code/%2E%2E/secret.txt` or a file of the repository no doc cites and the
  debt does not list, then the answer is 404 and holds none of its content.
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
- (WEB-43) Given the query `a`, then an `htmx` search gets an empty answer, and a submit the search page.
- (WEB-43, WEB-109) Given a box emptied, then the `htmx` search gets an empty answer and the panel closes.
- (WEB-109) Given more than 10 hits, then the panel shows the 10 of the search page and its link to the full page.
- (WEB-112) Given the last option active, when Down is pressed, then the first is active.
- (WEB-111) Given the focus in the question field of the ask page, when `/` is pressed, then it is typed there.
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

- (WEB-79) Given a review with no author (no `user.name`), then the page shows its status, comment and date only.
- (WEB-80) Given a feature marked `wrong`, then its row on the page of its domain carries the badge, and its use
  cases' rows on its own page carry none.
- (WEB-81) Given a feature validated, when the form sends `wrong` with the comment "misses the refunds", then
  `reviews.json` holds one review of it, wrong, with that comment.
- (WEB-81) Given a reviewed use case, when the form sends no status, then the use case has no review and the other
  reviews are kept.
- (WEB-84) Given a page of another site posting a form to `http://127.0.0.1:8730/domains/billing`, then its `Origin`
  names that site and the request is refused; one with neither `Origin` nor `Referer` is refused too.
- (WEB-84) Given `Origin: null` (a sandboxed frame, a `file://` page), then the request is refused.
- (WEB-87, WEB-88) Given a `reviews.json` holding invalid JSON, then the pages of the sections are still served, naming it.
- (WEB-92) Given a use case whose narrative is edited, when a later run rewrites it, then its page shows the edited
  narrative, the edit outdated as "rewritten" and the new generated narrative.
- (WEB-95) Given a use case shown as generated, when the form sends its name and description unchanged and a new
  narrative, then the edit holds the narrative only.
- (WEB-95) Given a feature whose name is edited, when the form sends the name emptied, then the feature has no edit.
- (WEB-94) Given a narrative of two lines typed in a textarea, ending with a line end, then the edit holds the two
  lines joined by a line feed alone, with no line end after them.
- (WEB-104) Given a section with no review, when its page is asked with `?saved=review`, then the form shows the
  notice: the parameter says a save happened, the page does not check it.
- (WEB-105) Given an edit form whose name and description were changed, when the save fails, then the form answered
  holds the changed name and description, not the text of the page.
- (WEB-89) Given an outdated review, when the form saves the same status again, then the review is no longer flagged
  outdated (REV-22).

- (WEB-118, WEB-120) Given a narrative holding `<script>alert(1)</script>`, then the page and the preview show it as
  text.
- (WEB-120) Given a narrative holding `[x](javascript:alert(1))`, then the preview shows `x` with no link.
- (WEB-122) Given a narrative shown on the page and in the preview of its edit, then no `id` of the page comes from
  it, so none is repeated.
- (WEB-123) Given `[guide](../../../../../guide.md#setup)` in a narrative whose page is under `docs/` and a collected
  `guide.md`, then the link leads to `/docs/guide.md#setup`.
- (WEB-123) Given a relative link that climbs above the repository root, or a root path (`/guide.md`), then it is
  shown as its text alone.
- (WEB-123) Given `%2E%2E/` in a relative link, then it climbs as `../` does; given an email autolink
  (`<ada@example.org>`), then it stays a `mailto:` link.
- (WEB-123) Given a run with no reconciliation of the collected docs, then the links still resolve from the docs dir
  of the config.

## Non-goals

- No resolution of a link of a narrative to another generated page (a feature, a use case): it is shown as its text
  alone. The answers of the ask page keep the rules of a collected doc.
- No preview of the name nor of the description: the pages show them as plain text. Without JavaScript the preview
  shows the narrative of the page and is not asked again, and the revert is sent with no confirmation.

- No synonym matching nor matching of a word being typed: the search, also as it is typed, is that of
  `retrodoc search` (`specs/docs-search.md`); a word is matched once typed whole.
- The delay of the search as it is typed is not adapted to the size of the run.
- The glossary shows no attributes nor associations of a concept, and has no page per concept.
- The files and images a collected doc links to are not served: only the collected Markdown docs are.
- No line of a file the docs don't cite is ever served: the files page shows only the path of such a file (ADR 0016).
- No syntax highlighting: the code is plain text with line numbers.
- The diagram is drawn by the browser: whether `mermaid` succeeds on an LLM-written diagram is not required, only that
  its source stays in the page.
- The actors page does not merge the spellings of an actor nor maps them to the actors list (`actors.yaml`).
- The rules page has no free-text filter: the search covers the rules (WEB-40).
- No authentication nor several repositories: the server is for the local user (a hosted mode needs its own ADR).
- No LLM call without `--ask` (ADR 0048, `specs/web-ask.md`): a review records a judgement and leaves the docs as they are; an edit (WEB-93) is an overlay over the
  generated text, which the artifacts keep (`specs/section-edit.md`).
- The edit form changes the name, the description and the narrative only: no steps, business rules nor glossary
  (`specs/section-edit.md`); no button per field brings one field back: emptying it does.
- No authentication of the reviewer: the author of a review is the `user.name` of the repository (REV-4), whoever
  sends the form.
- No reason of an outdated review in the badge of a list (WEB-90): the page of the section gives them.
- The look of the states of a form (the indicator, the notice that fades) is no requirement: it is checked by eye in
  `docs/dev/web-ui.md`; a test checks the markup that `htmx` reads, not what a browser draws.
- The name of the application is not asked of the LLM, nor read from the `origin` remote; the Markdown render and the
  MCP answers don't carry it.
- The home page links to the debt page (`specs/debt-view.md`) only, not to the audit dashboard
  (`specs/audit-dashboard.md`), which counts nothing; the header links to both. Nor does it show the external systems and the open questions of the
  brief: only its purpose and its users.
- The colours of the interface (the charter of the Meta Lambda site) are no requirement: they are checked by eye, not
  by a test.
- The docs are read once at start: the pages of a `generate` run while the server runs show after a restart of `ui`.
- The maps draw no use case: a domain of a large application has hundreds; they are counted on their feature. Nor do
  they draw use cases that call one another: the docs hold no such link.
- The layout of a map is that of `mermaid`, drawn by the browser: how it reads is checked by eye, not by a test.
