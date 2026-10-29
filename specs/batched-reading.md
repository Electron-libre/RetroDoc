---
id: BRD
status: accepted
adrs: []
---

# Parts of a long file in the batched readings

## Intent

The glossary, the entry points and their joint reading read many small files per LLM call, and a file too long for
one call is read in several parts whose findings are merged. Each part must be asked in a way the model answers for
it, and a part that gives nothing while the others of its file do must not go unnoticed: the inventory would silently
miss what that part declares.

## Requirements

- **BRD-1** (deterministic) Two parts of the same file must not be asked in the same LLM call: each part of a long
  file goes to a call of its own, which may also hold parts or files of other paths.
- **BRD-2** (deterministic) A file must be recorded only once every one of its parts has an answer, with the findings
  of all its parts merged.
- **BRD-3** (deterministic) The run must log a warning naming the pass, the file and the numbers of the parts that
  gave nothing when a part of a file the entry points pass reads gives no finding while another part of the same
  file gives some, unless the file is a business file.
- **BRD-4** (deterministic) The run must log one line naming the pass and the file, beside the warning of the
  unusable answer, when the answer for a call holding a single file or part is unusable; the pass leaves that file
  unrecorded.
- **BRD-5** (deterministic) The run must log the parts that gave nothing at the info level, not as a warning, for a
  business file or a file the glossary or the joint reading reads: a part of a model often holds no entity and no
  public method.

## Scenarios

- (BRD-1) Given a file cut in three parts of 4,000 characters each and a budget of 12,000 characters per call, when
  the batches are made, then the three parts are in three different calls, the first one possibly with the files
  before it and the last one with the files after it.
- (BRD-3) Given a file in three parts whose answers give 39, 0 and 0 findings, then one warning names parts 2 and 3.
- (BRD-3) Given a file in three parts that all give no finding, then no warning is logged (an empty file of its kind
  is not a loss).
- (BRD-4) Given a call holding several files whose answer is unusable, then it is retried file by file as before, and
  only a file whose own call is unusable is named.
- (BRD-5) Given a business model in five parts whose public methods are all in part 5, then an info line names parts
  1 to 4 and the run logs no warning.

## Non-goals

- Whether the model lists every entry point of a part: that depends on what the model writes; the warning makes a
  silent empty part visible, it does not prove completeness.
- Keeping the number of calls: a long file costs one call per part, even when its parts would fit one call together.
