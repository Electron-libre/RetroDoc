---
id: SED
status: accepted
adrs: [0044, 0045]
---

# Section edit

## Intent

A human corrects what the model wrote of a domain, a feature or a use case: its name, its description and, for a use
case, its narrative. The edits are one record kept in the repository, an overlay over the generated sections that
leaves them as they are beneath, so that the web form, the read model, the rendered Markdown and the MCP answers share
it and no `generate` loses it.

## Requirements

- **SED-1** (deterministic) An edit must name its section by its kind (domain, feature or use case) and its id, the
  id of the run history (RHI-7: the slug, `<feature>/<use case>` for a use case).
- **SED-2** (deterministic) An edit must hold only the fields it changes among the name, the description and the
  narrative; a field it leaves out is absent from the edit.
- **SED-3** (deterministic) An edit of the narrative of a domain or of a feature must be refused, and nothing saved:
  only a use case has a narrative.
- **SED-4** (deterministic) An edit that changes no field must be refused, and nothing saved.
- **SED-5** (deterministic) An edit whose name, description or narrative is blank (empty or whitespace only) must be
  refused, and nothing saved.
- **SED-6** (deterministic) An edit must hold as author the `user.name` of the git config of the repository, none
  when it is unset or the folder is not a git repository.
- **SED-7** (deterministic) An edit must hold the date it was made in RFC 3339, in UTC, to the second.
- **SED-8** (deterministic) An edit must hold the fingerprint of its section as generated (REV-6), the edits left
  out, when the edit was made.
- **SED-9** (deterministic) An edit must hold the hash of the content of each file its section cites (as REV-19
  lists them), as the file was when the edit was made; a file that cannot be read gets no hash.
- **SED-10** (deterministic) The edits must be saved in one file, `.retrodoc/edits.json`, outside `.retrodoc/cache/`.
- **SED-11** (deterministic) A `generate`, `--force` included, must not delete nor rewrite `.retrodoc/edits.json`.
- **SED-12** (deterministic) A save must write the whole file aside then rename it over the former one, so that no
  reader ever sees a file cut short.
- **SED-13** (deterministic) A file that cannot be read, or of a later format, must be reported as an error when the
  edits are loaded.
- **SED-14** (deterministic) A save must not overwrite a file that cannot be read or is of a later format: it fails
  and leaves the file as it was.
- **SED-15** (deterministic) An edit of a section must replace the previous edit of the same section whole, a field
  the new edit leaves out included.
- **SED-16** (deterministic) Removing the edit of a section must leave the section with no edit, the other edits
  kept.
- **SED-17** (deterministic) An absent file must load as no edit.
- **SED-18** (deterministic) The edits must be saved in a stable order (by kind, then id), so that a file under
  version control changes only on the lines of the edits that changed.
- **SED-19** (deterministic) An edit of a section the docs do not hold must be refused, and nothing saved.
- **SED-20** (deterministic) A file holding several edits of one section (two teammates' files merged) must load as
  the newest of them only.
- **SED-21** (deterministic) Edits saved at the same time by one process (two requests of `retrodoc ui`) must all be
  kept.
- **SED-22** (deterministic) Saving an edit or a review must leave the file of the other record
  (`.retrodoc/reviews.json`, `.retrodoc/edits.json`) as it was.

- **SED-23** (deterministic) The read model must hold the name, the description and the narrative of an edited
  section as its edit gives them, and a field the edit leaves out as generated.
- **SED-24** (deterministic) An edit of a section the docs no longer hold must be left out by the readers and kept in
  `.retrodoc/edits.json`.
- **SED-25** (deterministic) An edit recorded from a read model that already holds edits must hold the fingerprint
  and the hashes of its section as generated (SED-8, SED-9), the edits left out.
- **SED-26** (deterministic) A review must hold the fingerprint of its section as the read model shows it, the edit
  applied (REV-6), so that an edit made after the review makes it outdated (REV-20).
- **SED-27** (deterministic) `retrodoc ui` must serve, from the first request after `.retrodoc/edits.json` changed,
  the pages and the search with the edits the file holds then, without a restart.
- **SED-28** (deterministic) `retrodoc mcp` must answer each tool call with the edits `.retrodoc/edits.json` holds
  at that call.
- **SED-29** (deterministic) `retrodoc ui` and `retrodoc mcp` must warn of a file of edits that cannot be read or is
  of a later format, and show the docs as generated.
- **SED-30** (deterministic) The Markdown that `render` and `generate` write must show the fields of an edited
  section as its edit gives them, in place of the generated ones.
- **SED-31** (deterministic) The same artifacts and the same edits must render the same files, byte for byte.
- **SED-32** (deterministic) `render` and `generate` must fail with an error naming `.retrodoc/edits.json` when the
  file cannot be read or is of a later format, and write no doc.
- **SED-33** (deterministic) A blank field of an edit written by hand in `.retrodoc/edits.json` must be left out by
  the readers, the generated text shown in its place.

## Scenarios

- (SED-2) Given a feature whose name only is edited, then its edit holds the name and no description nor narrative.
- (SED-3) Given a domain, when its narrative is edited, then the edit is refused and no file is written.
- (SED-15) Given a use case whose name and narrative were edited by A, when B edits its description only, then the
  file holds one edit of it, by B, with the description only.
- (SED-16) Given a domain and a use case edited, when the edit of the domain is removed, then only the edit of the
  use case remains.
- (SED-14) Given an `edits.json` holding invalid JSON, when an edit is saved, then the save fails and the file is byte
  for byte the same.
- (SED-13) Given an `edits.json` of format version 2 read by a version 1 reader, then loading fails, naming the
  version.
- (SED-11) Given an edited section, when `generate --force` runs with no LLM call needed, then the edit is still in
  the file.
- (SED-8) Given a feature edited twice, the docs unchanged between, then both edits hold the same fingerprint.
- (SED-23) Given a use case whose narrative only is edited, then the read model shows the edited narrative and the
  generated name and description.
- (SED-25) Given a feature whose name is edited, when its description is edited next, then the second edit holds the
  fingerprint of the generated feature, the same as the first.
- (SED-26) Given a feature reviewed `validated`, when its description is edited, then the review is outdated as
  "rewritten".
- (SED-27) Given `retrodoc ui` serving a feature, when an edit of its name is saved, then the next page and the next
  search show the new name.
- (SED-32) Given an `edits.json` holding invalid JSON, when `render` runs, then it fails naming the file and the docs
  dir is as it was.

## Non-goals

- No edit of the steps, the business rules nor the glossary (decided with the user on 2026-11-05).
- No change to the generated artifacts: the edits are an overlay, applied by the readers.
- No history of the edits: a new edit replaces the former one; git keeps the history of a versioned file.
- No lock across processes: two processes saving at the same instant keep the last one's file whole (SED-12), not
  both.
- When an edit is outdated (`issues/edit_outdated.md`), and the form that writes it (`issues/web_edit_form.md`).
