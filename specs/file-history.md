---
id: FHI
status: accepted
adrs: [0016]
---

# File history

## Intent

The history of a file the docs cite, as the MCP tool `git_log` gives it to a coding agent and the web page of the
file shows it to a human: the commits that changed it, across its renames, so that the early history of a moved
file is not lost.

## Requirements

- **FHI-1** (deterministic) The history of a file must list, newest first, the commits that changed it under the
  previous names it was renamed from, as `git log --follow` does.
- **FHI-2** (deterministic) The commit that renamed the file must be marked `renamed from <old path>` in the output
  of `git_log` and on the web page of the file.
- **FHI-3** (deterministic) The history must not follow a copy: a file added as a copy of another one that is kept
  starts its history at the copy.
- **FHI-4** (deterministic) `read_source` and `git_log` must refuse a previous name of a cited file that the docs do
  not cite.
- **FHI-5** (deterministic) The history must leave merge commits out.
- **FHI-6** (deterministic) The history must not follow an empty file to a deleted empty file.

## Scenarios

- (FHI-1) Given a file committed as `old.rb`, renamed to `new.rb` without edits, then edited, then renamed and
  edited in the same commit to `app/new.rb`, then edited again, when the history of `app/new.rb` is asked, then it
  lists the five commits, newest first.
- (FHI-1) Given a limit lower than the number of commits, then the history stops at that limit, across the renames.
- (FHI-2) Given that history, then the two rename commits are marked with the path they came from, and no other
  commit is marked.
- (FHI-1) Given a file moved with 1,500 others in one commit, then its history goes on under its previous name.
- (FHI-3) Given `a.rb` copied to `b.rb` with `a.rb` kept, when the history of `b.rb` is asked, then it lists only the
  commits since the copy.
- (FHI-6) Given an empty `old/__init__.py` deleted and an empty `pkg/__init__.py` added in one commit, then the
  history of `pkg/__init__.py` starts at that commit.
- (FHI-4) Given `new.rb` cited and its former name `old.rb` not cited, then `git_log("old.rb")` and
  `read_source("old.rb")` answer that the file is not cited.

## Non-goals

- Following a file renamed and rewritten in the same commit below git's default similarity (50%).
- Following a file split into several files or merged from several.
- A rename made in a merge commit, or onto a path that existed before (the old file replaced in the same commit).
- On branches merged together, the commits of a branch that changed the file under its previous name after the
  other branch renamed it: one path is tracked along the walk, as `git log --follow` does.
- Bounding the walk of a file that rarely changes on a large repository (measured, not bounded, by this capability).
