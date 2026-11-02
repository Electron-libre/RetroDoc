---
id: RND
status: accepted
adrs: []
---

# Removal of the generated pages no run produces any more

## Intent

When an item is renamed, or every item at once after a change of `[output] language`, `generate` and `render` remove
the pages they wrote under the old names, so the docs folder holds only the current tree, while a page someone edited
since is kept and reported.

## Requirements

- **RND-1** (deterministic) `generate` and `render` must record, in `_retrodoc/rendered-pages.json`, the content hash of
  each Markdown page they write or leave unchanged under `functional/`.
- **RND-2** (deterministic) Each Markdown page under `functional/` must be removed when the run no longer produces it
  and its content still has the hash recorded for it, and listed in the preview on a line `remove  <path>`.
- **RND-3** (deterministic) Each Markdown page under `functional/` must be left in place and listed under "Not produced
  by this run" as before when the run no longer produces it and its content differs from its recorded hash or has no
  recorded hash.
- **RND-4** (deterministic) A folder under `functional/` that the removals leave empty must be removed.
- **RND-5** (deterministic) `render --dry-run` must list the pages it would remove and remove none.
- **RND-6** (deterministic) A second `render` with the same artifacts and configuration must write and remove no file,
  the record included.

## Scenarios

- (RND-2) Given a domain renamed in the saved artifacts since the last `render`, when `render` runs, then the folder of
  the old name is gone and the folder of the new name is written.
- (RND-3) Given the page of a renamed feature edited by hand since the last `render`, when `render` runs, then it stays
  and is listed under "Not produced by this run".
- (RND-3) Given docs written by a version without the record, when `render` runs, then no page is removed.

## Non-goals

- Removing files outside `functional/`, or files other than Markdown pages.
- Keeping a hand edit of a page the run still produces: it is overwritten, as before.
- A flag to remove the pages that were edited by hand.
