# 0017. Place the files a clustering left unassigned with a second LLM call

Status: Accepted

Refines 0004 (the coverage guarantee stays; this adds a step before its "uncategorized" bucket).

## Context

The first smoke run on a small Ruby repo (9 source files) ended with a `WARN`: the clustering had assigned
sub-domains to three single files and no folder, so the six other files of the same directory fell into
"uncategorized", got no feature and no use case, and the code holding the core of the application was
absent from the docs. Coverage held by construction (0004), but the repair threw away the information
that mattered. The cause was in the prompt (file paths in the surface section made the model answer with
files instead of the module paths it was asked for, fixed separately), but the answer varies from one call
to the next, so a prompt fix alone can't be trusted to remove the case.

## Decision

After the mechanical expansion of the modules to files, the files still unassigned are sent, with their
summaries and the domains and sub-domains found, in one extra call (60 files per call at most). The model
answers `{path, domain, sub_domain}` pairs; only a file that was asked about, an existing domain and an
existing sub-domain are honoured (an unknown sub-domain falls back to the domain). An unparseable answer
(after the usual single retry) or an invalid pair leaves the file unassigned, and the coverage repair of
0004 buckets what remains into "uncategorized". The result is saved in `domains.yaml`, so the existing
input fingerprint makes a rerun reuse it without the extra call. An LLM transport failure fails the run
like the clustering call does, rather than saving a degraded map that the fingerprint would then keep.

Rejected: inheriting the domain of the files of the same folder. It only works when they all agree, which
is not the case that triggered this (three different sub-domains in one folder). Rejected too: asking for
the whole clustering again, which costs a full call and can fail the same way.

## Consequences

One more call, only when the clustering is incomplete. "Uncategorized" now holds only what the model could
not place. The placement of a file rests on its summary, not on its code.
