---
id: HPG
status: accepted
adrs: [0039, 0043]
---

# History page

## Intent

The page `/history` of `retrodoc ui` lists the runs the history saved (`specs/run-history.md`) and compares two of
them, so whoever audits the docs sees which domains, features and use cases a `generate` added, removed or made worse.

## Requirements

- **HPG-1** (deterministic) The history page, `/history`, must list the saved snapshots (RHI-13), newest first, each
  with the date and time it ended and its zone (`UTC` for a UTC time), the number of its file after it when several
  runs ended in the same second (RHI-1), and the first 7 characters of its commit, or a line saying it had none.
- **HPG-2** (deterministic) With no `?from=` nor `?to=`, the history page must compare the newest snapshot (the "to"
  run) with the one before it (the "from" run).
- **HPG-3** (deterministic) The history page must compare the two snapshots `?from=` and `?to=` name by their file name
  without `.json`, `from` as the earlier state even when it is the newer run.
- **HPG-4** (deterministic) A `from` or a `to` that names no saved snapshot must be replaced by its value of HPG-2,
  with a line saying the run asked for is unknown.
- **HPG-5** (deterministic) With two saved snapshots or more, the list of HPG-1 must be a form that gives each run a
  radio choice `from` and a radio choice `to`, the compared runs checked, that sends them to `/history` as `?from=`
  and `?to=`: on any change (`hx-trigger="change"`, its URL pushed, the form and the difference swapped), its submit
  button inside `<noscript>`.
- **HPG-16** (deterministic) With two saved snapshots or more, the history page must show a link that swaps the
  compared runs, to `/history?from=<to>&to=<from>` of the runs compared.
- **HPG-6** (deterministic) The history page must list as added the sections (domain, feature, use case, by their kind
  and id of RHI-7) of the "to" run that the "from" run lacks, grouped by kind, in the order of the "to" run.
- **HPG-7** (deterministic) The history page must list as removed the sections of the "from" run that the "to" run
  lacks, grouped by kind, in the order of the "from" run.
- **HPG-8** (deterministic) The history page must list as degraded each section of both runs whose confidence or
  business language is lower in the "to" run by at least `0.2`, with
  the score that fell, its value in each run and the fall.
- **HPG-9** (deterministic) A score absent from either run must not make its section degraded.
- **HPG-10** (deterministic) The history page must list as newly set apart each use case of both runs set apart under
  "Administration and technical" (RHI-8) in the "to" run and not in the "from" run, with the reasons of the "to" run.
- **HPG-11** (deterministic) The history page must list as no longer set apart each use case of both runs set apart
  in the "from" run and not in the "to" run.
- **HPG-12** (deterministic) Each section of the difference must be a link to its page when the docs the interface
  serves hold it, and its name as text, marked as having no page, otherwise.
- **HPG-13** (deterministic) When the two compared runs differ by none of HPG-6 to HPG-11, the history page must say
  their sections do not differ.
- **HPG-14** (deterministic) With fewer than two saved snapshots, the history page must list them and say the
  difference needs two runs of `retrodoc generate`.
- **HPG-15** (deterministic) The "Audit" group of the header of every page must link to the history page, with a title
  saying what it holds.

## Scenarios

- (HPG-3) Given `?from=` naming the newer run and `?to=` the older one, then the page compares them as asked: what
  the newer run added is listed as removed.
- (HPG-3) Given the same run as `from` and `to`, then the page says their sections do not differ.
- (HPG-2, HPG-3) Given `?to=` alone, naming a saved run, then `from` is the run before the newest, as HPG-2 says.
- (HPG-6) Given a use case moved to another feature, then it is removed under its former id and added under the new
  one.
- (HPG-8) Given a confidence of `0.9` then `0.75`, then the use case is not degraded; given `0.9` then `0.6`, it is,
  by `0.3`.
- (HPG-9) Given a use case with a business language in the "from" run and none in the "to" run, then it is not
  degraded on it.
- (HPG-12) Given a section of the "from" run removed since, then it is shown as text, with no link.
- (HPG-4) Given a single saved run and `?from=` naming none, then the page says the run asked for is unknown and
  that the difference needs two runs.
- (HPG-1) Given two runs ended in the same second, then the second is listed with its number, `(2)`.
- (HPG-8) Given a fall shown in rounded percents, then the fall shown is the rounded fall, not the difference of the
  rounded percents.
- (HPG-14) Given a repository with no run yet, then the page says the difference needs two runs.
- (HPG-1) Given a history file that cannot be read, then it is not listed (RHI-13).

## Non-goals

- No CLI command: the difference is a web page (chosen with the user on 2026-11-04).
- No section that got better: the page lists what was added, removed or made worse; the trends of the overall
  figures are on the audit page (`issues/done/web_run_trends.md`).
- No difference of the figures (overall scores, coverage, debt, cost): the trends show them.
- No matching of a section renamed or moved: a section is the same in two runs only by its kind and id.
- No business rule, glossary concept nor actor: the snapshot holds none (RHI-7).
- The snapshots are read once at start, like the docs: a `generate` run while the server runs shows after a restart
  of `ui`.
