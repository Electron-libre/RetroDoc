---
id: RFD
status: accepted
adrs: []
---

# Routes a routing file declares for other files

## Intent

A routing file (`config/routes.rb`) declares the routes that the controllers of the application also give as
entry points. Read alone, it gives those routes a second time, and the later passes turn each copy into a feature or a
use case of its own. The later passes must work from one copy of a route: the one of the file that handles it.

## Requirements

- **RFD-1** (deterministic) The paths an `http_route` entry names must be read from its name: every word that starts
  with `/`, with its parenthesised optional parts and the extension of its last segment dropped, a trailing `/`
  ignored, and every placeholder segment (`:id`, `{id}`, `<id>`, `<int:id>`, `[id]`, `*rest`) read as the same
  placeholder.
- **RFD-2** (deterministic) A path of one segment must also be read as a sibling of the path before it when it
  follows a longer path in the same name and no entry of another file covers it (`/unvote` after
  `/stories/:id/upvote` is `/stories/:id/unvote`).
- **RFD-3** (deterministic) A path must be covered by an `http_route` entry when its segments are the first segments
  of one of the paths of that entry, or all of them; the root path `/` is covered by `/` only.
- **RFD-4** (deterministic) A file must be a routing file when at least 3 other files have an `http_route` entry
  with a path equal to one of its paths, and when at least half of its `http_route` entries have a path covered by an
  entry of another file.
- **RFD-5** (deterministic) An `http_route` entry of a routing file must be left out of the entry points the later
  passes work from when each of its paths is covered by an entry of another file that is not a routing file.
- **RFD-6** (deterministic) An entry must be kept when it names no path, when it has a path no other file covers,
  or when its file is not a routing file.
- **RFD-7** (deterministic) The saved `entry-points.yaml` must keep every entry, so that the choice is made again at
  each run from the current inventory.
- **RFD-8** (deterministic) The ids (`<file>#<name>`) of the entries left out must be returned with the inventory
  the later passes work from, in the order of the files and of their entries, so that `generate` logs them.

## Scenarios

- (RFD-3) Given a routing-file entry `/invitations (resources)` and a controller entry `GET /invitations/request`,
  when the coverage is computed, then `/invitations` is covered.
- (RFD-3) Given a controller entry `GET /invitations/request` and a routing-file entry `/invitations`, then
  `/invitations/request` is not covered by the routing-file entry (it is longer).
- (RFD-5) Given `GET /signup, POST /signup, /signup/invite` in `config/routes.rb` and `GET /signup`,
  `GET /signup/invite` in `signup_controller.rb`, then the routing-file entry is left out and the controller entries
  stay.
- (RFD-6) Given `GET /u, /u/:username` in `config/routes.rb` (redirects) and no other file giving `/u`, then it stays.
- (RFD-6) Given `GET /api/bookmarks/` in `bookmarks/api/routes.py` (view sets, the actions are in the file) and
  `GET /bookmarks/` in `bookmarks/views/bookmarks.py`, then nothing is covered and both stay.
- (RFD-5) Given two files that each cover paths of the other and of a third file, then both are routing files and
  neither loses an entry to the other.
- (RFD-4) Given a controller that gives `GET /` and `GET /t/:tag`, also declared by `config/routes.rb` and by one
  other controller, then it is not a routing file (2 other files), even if most of its entries are covered.

## Non-goals

- Reading the route declarations with a parser of each framework: the names the LLM gives are all that is read.
- Linking a route of a routing file to the action that handles it under another path (an alias such as
  `/~:username/ban` for `/users/:username/ban`): such a route stays.
- Changing the entry points prompt or the saved readings of the entry points pass.
