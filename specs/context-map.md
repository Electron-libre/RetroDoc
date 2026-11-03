---
id: CTX
status: accepted
adrs: [0029, 0031]
---

# Map of the bounded contexts

## Intent

`retrodoc contexts` maps a large application into bounded contexts that are documented one run at a time. A user
who redoes the map (`--force`) must keep the contexts already documented and get a map close to the previous one,
whose contexts hold business, not technical layers.

## Requirements

- **CTX-1** (deterministic) The map call and the split call must be sent at temperature 0 when the configuration sets no temperature for the `contexts` pass.
- **CTX-2** (deterministic) A source file whose role is `infra` or `config` must go to the support before the map call: it is in no context and no folder the call is shown is made of such files only.
- **CTX-3** (deterministic) A map made with `--force` over a saved map must give a new context the slug and the name of the saved context that holds more than half of its files, when more than half of that saved context's files are in it; a saved slug goes to one new context at most.
- **CTX-4** (deterministic) A context of a single source file must be merged, before the map is saved, into the context under 150 files that holds the most files of its folder; with none, it must go to the support unless its file is an entry point file, which keeps its context; a map whose contexts all hold one file must be kept as it is.

## Scenarios

- (CTX-1) Given `[llm.passes.contexts] temperature = 0.7`, when the map is made, then the call is sent at 0.7.
- (CTX-2) Given a folder that holds only migrations and one that holds migrations and a model, when the map is made, then the first is not listed to the call and the second is listed with the model only.
- (CTX-3) Given a saved context `billing` of 10 files and a new context holding 6 of them among 11 files, when the map is made with `--force`, then the new context is `billing`.
- (CTX-3) Given a saved context `billing` documented and done, when the map made with `--force` gives its same files to a context `invoicing`, then that context is `billing` and stays done.
- (CTX-3) Given a saved context of 10 files split into two new contexts of 5, when the map is made with `--force`, then neither takes its slug.
- (CTX-4) Given a one-file context whose folder holds no file of another context, when the map is made, then the file goes to the support.
- (CTX-4) Given a one-file context of an entry point file alone in its folder, when the map is made, then it stays a context.
- (CTX-4) Given a one-file context whose folder holds files of a context of 150 files or more only, when the map is made, then the file goes to the support.
- (CTX-4) Given a map of two one-file contexts and no other, when the map is made, then both stay.

## Non-goals

- That two maps of the same inputs are the same: the hosted model gives no fixed answer even at temperature 0; the
  spread is measured, not required.
- That the names of the contexts fit their files: only a model's answer decides it, read by a person.
- Changing a saved map without `--force`: a saved map is reused as it is, hand edits included.
- Setting the view layers (UI components) aside before the call: they serve a business context, the call gives them
  to it.
