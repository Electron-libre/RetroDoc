---
id: CSP
status: accepted
adrs: [0029]
---

# Split of a large bounded context

## Intent

A bounded context of the map (`retrodoc contexts`) must fit one run. A context over the limit is cut by one
more LLM call into parts, by business family rather than by layer, so that each run documents a vertical slice.

## Requirements

- **CSP-1** (deterministic) A context of more than 150 source files must be replaced by the parts the split call gives, every one of its files in exactly one part.
- **CSP-2** (deterministic) A part still over 150 files must be split again once at most: no context is sent to more than two split calls.
- **CSP-3** (deterministic) A context whose listing holds fewer than three groups and folders must not be sent to a split call.
- **CSP-4** (deterministic) A context left over 150 files must be named in a warning with its file count and the reason it stayed whole: too few entries to cut, an unusable answer, fewer than two parts, or two splits made already.
- **CSP-5** (deterministic) A name of the files in no group must become a family group of the split only when it shows in at least two and at most eight folders and names at least three files.

## Scenarios

- (CSP-1) Given a context of 151 files whose split answer forgets a folder, when the map is built, then that folder's files go to the first part.
- (CSP-2) Given a split whose part is still over 150 files and whose second split leaves a part over 150 files, when the map is built, then no third split call is made and CSP-4 warns with "split twice already".
- (CSP-3) Given a context of 200 files in one group and one folder, when the map is built, then no split call is made and CSP-4 warns with "too few entries".
- (CSP-4) Given a context the split call answers with a single part, when the map is built, then the warning names that context and the reason "fewer than two parts".
- (CSP-5) Given a name in two folders with only two files, when the context is split, then it is not a family and its files stay orphans.
- (CSP-5) Given a name in nine folders, when the context is split, then it is taken for a step every family has, not a family.

## Non-goals

- That the parts or their names fit the business: only a model's answer decides it, read by a person.
- That two runs of `contexts --force` give the same map: the hosted model has no fixed seed.
- Splitting a context already documented: a new map comes only from `retrodoc contexts --force`.
