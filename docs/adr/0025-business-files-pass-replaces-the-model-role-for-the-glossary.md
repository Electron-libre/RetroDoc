# 0025. A dedicated pass locates the business files; the glossary stops reading the `model` role

Status: Accepted (supersedes 0018; refines 0008, which stays in force)

## Context

ADR 0008 builds the glossary from the files the roles pass classes `model`, and ADR 0018 added a fallback on
`logic` then `entrypoint` files when that gave nothing. The `model` role is a layer label decided by an LLM
that sees only the file tree, and it does not say where the business is. Five runs of the roles pass per
repository (`issues/locate_business_files.md`): on a small Ruby library of plain classes `model` matched the
business files in 2 runs of 5; on a Django application `models.py` was classed `model` in 2 runs of 5 and the
migrations (about 54 files) in all 5. The fallback picked 30 files by path, not by how likely they are to hold
the business.

## Decision

A pass of its own, `business_files/`, runs after the roles (it needs the stack) and the product brief (ADR
0019) and before the glossary. One LLM call over the file tree, the stack, the brief and cheap evidence per
directory (files, commits, most changed file names), the doc titles and the vocabulary of the tests answers a
ranked list of at most 40 files and directories, each with a reason; paths that are no source file are dropped.
It is saved as `business-files.yaml`, hand-editable and reused while the shape of the source tree and the brief
are unchanged (`retrodoc business-files [--force]`). The glossary reads the files of that list (directories
expanded, 60 files at most) with a prompt that keeps business classes only. The roles keep the mechanical
classification (tests, entry points, config); `model` stops being the glossary's source and is used only when
no list exists. The ADR 0018 fallback is removed: it is the same assumption as the `model` role, in a weaker
form.

Rejected: tuning the roles prompt (the answer stays random), and keeping the fallback as a safety net (it hid a
failing business pass and read files chosen by path).

## Consequences

One more LLM call per run on a new or changed source tree, none otherwise. Measured on the same two
repositories, the pass finds 8 of 8 business files on the library in 4 runs of 5 and 6 to 12 of 13 on the
Django application (the `model` role: 0 to 1), but it still lists views, serializers and API routes in most
runs, so the glossary of such an application holds presentation classes (precision about 0.5 against a
hand-made list); the prompt was tightened once on the same repository, so these figures are not an
independent test. No quality benchmark of the final documents was run. If the pass fails and no model file is
recognized, the glossary is empty and `generate` logs it; fix by editing or forcing `business-files.yaml`. The
entry points still read the `entrypoint` role, and the actors read authorization code: the same question may
be asked of them later. The tree-shape hash of `sources/` counts the generated docs, like the first version of
this pass did; not fixed here.
