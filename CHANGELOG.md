# Changelog

The notable changes of each version, newest first ([Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
[Semantic Versioning](https://semver.org/)). Each section is written by `just release <version>` from the commits,
then reread before the release is committed.

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
