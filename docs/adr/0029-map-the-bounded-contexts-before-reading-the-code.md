# 0029. Map the bounded contexts before reading the code

Status: Accepted

## Context

A large application cannot be documented in one run (`issues/done/validate_on_large_application.md`): the
reading of the business files stops at 60 files (`business_files::MAX_FILES`) and takes the first of the
first folder in path order, so the glossary, and on the `surface` path the domains, rest on an arbitrary
corner. The user wants several complementary runs, none losing work, each on a vertical slice of the
application (a bounded context with its entry points, models, services and rules), not on a layer. That
needs the slices to be known before the code is read, and to stay the same from one run to the next: the
domains of the `surface` path (ADR 0020) are built from what the reading found, and a domain change
redoes the features and use cases of every unit it touches.

Measured with no LLM call (`examples/context_map_measure.rs`): grouping the source files around the names of
the entry point files gives real slices on a large Rails application (most of the 15 largest groups are a
business object with its controllers, models, services and policies), but also technical groups, pairs that
are one concept, and a third of the files in no group; on a Django application the names say nothing.

## Decision

`contexts` builds the map in two steps. The mechanical step groups the source files around the names of the
entry point files and attaches the other files by their name or the name of one of their directories (the
longest name first, one group per file). One LLM call then reads the groups (id, size, sample of files) and
the folders holding orphan files, under the product brief, and answers contexts (groups merged, a name, a
description), a support set of technical groups and folders, and a context for each orphan folder. A group
the answer forgets becomes a context of its own; an unreadable answer gives one context per group and is not
saved.

`contexts.yaml` keeps the rules (group names and folders per context), not the file lists. The files of a
context are found again from the current tree by `ContextMap::resolve`, so a new file joins a context by the
same rules, a hand edit takes effect at once, and a file in no rule is support. The map is reused as it is
until `retrodoc contexts --force`: the code changing never changes it, which is what lets a context done
stay done. Rejected: a map recomputed from the tree at each run (the slices would move under the
fingerprints), and a stored list of files (stale as soon as a file is added).

A context of more than 150 files is too big for one run (the central context of an application often
regroups several sub-domains: editing, signing, amending a contract…), so one more call splits it into
two to eight sub-contexts: its groups and its orphan folders, listed by sub-folder, are partitioned, and
what the answer forgets or sets aside stays with the first part. The sub-contexts replace it in the map
(siblings, no nesting); a failed or unusable split leaves the context whole.

A file belongs to one context. A shared file is read once, with the context that owns it; the others refer
to it by name. The map is for the `surface` path; the `files` path is unchanged.

## Consequences

* `[pipeline] domains_from = "contexts"` documents an application a slice at a time. A run (`generate
  [--context a,b] [--max-files N]`) reads the files of its contexts, makes each context a domain with no LLM
  call, and runs features, use cases, rules and confidence on every feature: the earlier ones are found up to
  date and cost nothing. For that, the readers keep the inventories of the other files (`Retention::Others`),
  the vocabulary of the prompts is the domain's own, the actors are identified once and kept
  (`retrodoc actors --force` asks again), and the entry points of a domain are those of its files. The
  `business-files` pass is not run in this mode (the files of the contexts are the business files). A
  context is done while the hash of its files, and of the files read for it, still matches; a file edited in a
  context already done is taken up again when that context is named or marked pending by the change.
* An application with no entry point file is one context (the whole application).
* One LLM call over about 140 groups on the large application: 8k tokens in, 1 to 1.5 minutes locally.
* First measure on that application. A local model (two prompts): 19 to 21 contexts but 44% to 52% of the
  source files in the support, whole layer folders and one-job groups sent there against the instruction. A
  hosted model (about $0.004): 18 contexts that read as business areas and 32% of the files in the support,
  mostly migrations, assets, UI components and helpers. The map is only as good as that answer, so orphan
  files should also be placed without it (by the references of the files of a context), to be done before
  the reading depends on the map.
* The quality of the merge depends on the names: stacks that name files after their layer (the Django case)
  give few groups, and the call then relies on the folders. To be measured on a second repository.
* The map is editable: a person who knows the application corrects the slices once and every later run
  follows.
