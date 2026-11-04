# Changelog

The notable changes of each version, newest first ([Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
[Semantic Versioning](https://semver.org/)). Each section is written by `cardinal run release <version>` from the commits,
then reread before the release is committed.

## [0.7.0] - 2026-11-04

### Milestones

- Release and code hygiene
- Measured quality at the current code
- Explore the documentation debt and audit it

### Added

- reject wrong judge pairs and count the unaudited ones
- set aside the features the features pass marks technical
- check the evidence of the benchmark reference
- ask the map of the contexts at temperature 0
- leave the technical files out of the map of the contexts
- keep the slugs of the contexts a forced map matches
- merge the one-file contexts of the map
- leave the domains with no feature out of the docs
- count only the domains a feature belongs to
- spread the files of a layer context to their users
- spread the catch-all contexts of the map
- propose the saved support to a forced map call
- pair the unpaired items with the covered reference ones
- add the judge's second call to a saved judgement
- list the debt of the docs as typed items
- show the debt of the docs on a filterable page
- give the figures of the audit of the docs
- show an audit dashboard of the docs
- keep what the docs leave out in the read model
- list the code no doc covers and the project docs in the debt
- show the code no doc covers and the project docs in the debt
- rank the debt items by weakness, churn and entry points
- show the priority of each debt item, churn read at start
- weigh the debt items by kind, commits of the busiest file

### Fixed

- resolve an id only when every segment given matches
- make the docs builders order-free and the lookups linear
- keep the source access through From and share the use case id
- share one list item and keep it on one line
- show an inverted line range with the lower line first
- fail the render when two items fold into one path
- order the cited lines once for the docs, MCP and web
- resolve a business path with a trailing slash to its file
- resolve a trailing-slash file the same way when reading
- report the released version 0.6.0 and guard it
- read the newest changelog heading whatever its form
- correct the judge audit of the benchmark baseline
- refuse a pair both kept and rejected in matches.yaml
- lint the default build of the pipeline, one judgement path
- keep external exchanges out of the technical features
- treat operations commands and jobs as technical features
- keep slice merges of technical features in the docs
- make the reference evidence check refuse stale inputs
- keep the one-file entry point contexts of the map
- tie the number of domains to the scoring version
- place each file of a layer context once, before the split
- stamp version 4 on a run judged without the second call
- count the debt items at each request of the home page
- correct the figures of the audit dashboard
- correct the debt view of the code after review
- count no commit for a debt item whose feature left the run
- make the scenarios of the debt priority reachable and tested

## [0.6.0] - 2026-11-02

### Milestones

- Web interface for a first-time reader
- Docs in the reader's language

### Added

- print the whole request of the reading passes
- print the whole request of the other LLM passes
- judge the saved rules again on a judgement change
- follow renames in the history of a cited file
- name the documented application in the web interface
- open the home page on where to start reading
- group the header links by how a reader uses them
- link each use case to its neighbours in its feature
- dress the interface in the Meta Lambda charter and credit
- draw the domains as a map linked by their shared concepts
- draw the map of a domain with its actors and concepts
- write the generated docs in the language of the config
- match doc sections by the identifiers of an item
- warn when no doc section matches in another docs language
- write the fixed text of the docs in French
- write the frame of the pages in the language of the docs
- write the pages of the items in the language of the docs
- remove the pages no run produces any more
- find the language tag from native names and regional tags

### Fixed

- follow exact renames on big moves and skip empty files
- log the ignored business paths at info
- read text-only steps and drop empty use cases at info
- warn only of what the surface passes really lose
- keep the saved order of the features a rerun reuses
- keep the saved feature order in the partial saves
- write a saved brief again when the docs language changed
- search routes without their verbs in the reconciliation
- remove the stale pages safely
- read a region only as two letters or three digits

## [0.5.0] - 2026-11-02

### Milestones

- Browse the docs in a web interface

### Added

- serve the domains of the last run in a local web interface
- add the ui command to browse the docs in a browser
- show the page of a domain with its existing docs
- show the page of a feature
- show the page of a use case with its diagram drawn
- show the cited code of a file with its history
- show the tree of the source files with their citations
- show the collected docs of the repository as pages
- show the glossary with the use cases of each concept
- search the docs from every page
- list the business rules with filters by kind and place
- show which actors take part in the use cases of each feature

### Fixed

- refuse foreign hosts and keep the pages off the async workers
- show the rationale of the rules and encode the doc links
- refuse a range past the end and link to the next lines
- strip doc front matter and answer htmx with no run
- keep the whole page on an htmx history restore

## [0.4.0] - 2026-11-01

### Milestones

- Business rules quality

### Added

- replay the rules judgement on the rules a run stated
- show the rules judgement what is not business
- give the rules replay the put-backs of the source run
- leave out of the search the entries sharing only common words

### Fixed

- give the rules replay the evidence generate gives
- read only the [rules] of the source run for the put-backs
- name the unknown words in the not-documented search answer
- keep every name of the prompts and lists on one line

## [0.3.0] - 2026-10-30

### Milestones

- A large application, one bounded context at a time

### Added

- `contexts` warns of each context left over the size limit with its file count and the reason it was not split.

### Fixed

- The actors pass reads every authorization file within its budget, a long one as an excerpt, and names the files
  the budget leaves out.
- The authorization code sent to the actors pass stays within its budget, markers of the excerpts included.
- Each context slug stays unique across a split level.

## [0.2.0] - 2026-10-30

### Milestones

- Language and framework agnostic

### Added

- `scan` and `roles` count the files of the languages `roles.yaml` promotes to source.
- The files no rule of `roles.yaml` matches get a role from their paths, in one more call.

### Fixed

- The roles given to the unmatched files are kept out of `roles.yaml`.
- Templates, markup, stylesheets and data are never promoted to source; only the denied extensions a file holds are
  reported.
- Singular, BDD and Android variant test layouts are read as tests.
- Blade, PHP and Drupal view templates (`.blade.php`, `.html.php`, `.tpl.php`) and CakePHP `.ctp` templates are
  kept out of the source files.

## [0.1.0] - 2026-10-30

### Milestones

- Foundation
- Repo map
- Domains
- Features, use cases and diagrams
- Confidence and debt report
- Writing and CLI ergonomics
- Business-level documentation
- Scalability and cost control
- MCP server over the generated docs
- Top-down redesign

First numbered release.
