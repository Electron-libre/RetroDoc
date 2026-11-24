---
id: WAC
status: accepted
adrs: [0039]
---

# Actors page of the web interface

## Intent

The actors pages of `retrodoc ui` tell whoever takes an application over who its actors are (`/actors`) and, for the
actor they pick (`/actors/<id>`), what it starts and takes part in, each consulted in about one screen on an
application of any size, the use cases one click away.

## Requirements

### Behaviour

- **WAC-1** (deterministic) The actors page must show an index of the actors, the human ones then the system ones under
  their own heading, each sorted by name ignoring case, each a link to its page followed by the number of use cases
  it starts and the number of the others it takes part in, and no use case.
- **WAC-2** (deterministic) Each actor of the index must have a page, `/actors/<id>`, headed by its name, its kind and
  the two numbers of WAC-1, with a link back to the index.
- **WAC-3** (deterministic) The page of an actor must list the domains of the use cases it starts (its primary actor) or
  takes part in (the actor of one of their steps), each a link to its page with the number of its features and of
  those use cases it shows, and under each domain those features, each a link to its page with the number of use cases
  the actor starts and takes part in there, domains and features in the order of the run.
- **WAC-10** (deterministic) The page of an actor must fold under each feature the use cases of WAC-3, closed when the
  page opens, each a link to its page, in the order of the run, those it starts first and marked.
- **WAC-4** (deterministic) The page of an actor must list after its domains the permission rules that apply to one
  of its use cases, each a link to its item on the page of its feature.
- **WAC-5** (deterministic) The map of a domain (`/map/<domain>`, WEB-73) must link each of its actors to the page of
  that actor.

### Contract

- **WAC-6** (deterministic) The actors must be grouped by their exact name, and be the primary actors of the use cases
  and the actors of their steps; a primary actor no step names is a human one, and a name the steps give both kinds is
  an actor of each kind whose primary use cases are started by the human one only.
- **WAC-7** (deterministic) The id of an actor, the last segment of the path of its page and the `id` of its entry in
  the index, must be `actor-<slug>` for a human one and `system-<slug>` for a system one, the slug its name in lower case with each run of characters other than letters
  and digits (of any script) read as one `-`, none at either end (`actor` for a name with none left), and `-2`, `-3`… after
  an id an earlier actor of the index or a list of the index already took; the index lists the human actors under the id `human-actors` and
  the system ones under `system-actors`.
- **WAC-8** (deterministic) The actors page and the page of an actor must not hold a table: no column per feature, nor
  per domain.
- **WAC-9** (deterministic) `/actors/<id>` must answer 404 with the page of a missing item when no actor of the run
  has that id, and the page that says to run `retrodoc generate` when the repository has no run.

## Scenarios

- **WAC-H1** (happy; WAC-1, WAC-2, WAC-3, WAC-10, WAC-4) Given a run whose `Clerk` starts `pay-by-card` and takes a step of
  `refund` and of `ship`, when the reader opens `/actors`, then the index lists `Clerk` under the people with
  "starts 1" and "takes part in 2" and no use case; when they follow `Clerk`, then `/actors/actor-clerk` lists
  `Billing` with 1 feature and 2 use cases, under it `Pay an invoice` with "starts 1 · takes part in 1" and, folded,
  `pay-by-card` marked then `refund`, then the domain and feature of `ship`, then the permission rules of the three.
- **WAC-H2** (happy; WAC-5, WAC-7) Given the map of a domain whose feature `Clerk` starts a use case of, when the
  reader follows `Clerk` on it, then they land on `/actors/actor-clerk`.
- (WAC-6, WAC-7) Given `Clerk` and `clerk`, then they are two actors, `#actor-clerk` then `#actor-clerk-2`.
- (WAC-6) Given a primary actor no step names, then it is a human actor; given a name the steps give both kinds, then
  it is listed under each, and as a primary actor it starts its use cases in the human group only.
- (WAC-9) Given `/actors/actor-nobody` on a run with no such actor, then it answers 404; given a system actor
  `Clerk` and no human one, then `/actors/actor-clerk` answers 404 and `/actors/system-clerk` its page.
- (WAC-1, WAC-10) Given an actor that both starts a use case and takes part in one of its steps, then the use case is
  listed once, marked, and counted among those it starts only.
- (WAC-3) Given a feature none of whose use cases names the actor, then the page of the actor does not show it,
  nor a domain none of whose features it shows.
- (WAC-3) Given a feature whose domain the run does not list, then it is listed under the slug of that domain, with no
  link.
- (WAC-6) Given a use case whose feature is not in the run, then its actors are not listed for it, nor at all when it
  is the only use case that names them.
- (WAC-4) Given an actor none of whose use cases a permission rule applies to, then its page lists no rule.
- (WAC-7) Given the name `Payment gateway (Stripe)`, then its id is `system-payment-gateway-stripe`; given `???`,
  then it is `actor-actor`; given `Équipe support`, then it is `actor-équipe-support`.
- (WAC-7) Given a system actor named `Actors`, then its id is `system-actors-2`, the list of the systems holding
  `system-actors`.
- (WAC-1) Given a run with no use case that names an actor, then the page says no actor was found.
- (WAC-1) Given a repository with no run yet, then the page says to run `retrodoc generate`.

## Non-goals

- The search does not list the actors: the index and the map lead to their pages.
- No overview of the actors by domain (actors × domains): the index gives the numbers of each actor, its page the
  detail by domain.
- The generic system actors (a "System" that takes part in almost every use case) are not recognised: they are
  listed with the other system actors, under their heading after the people, a line of the index like the others.
- The page does not merge the spellings of an actor nor maps them to the actors list (`actors.yaml`).
- The page of an actor is not paged; the folds of its features are not remembered between two visits, nor
  opened all at once by a control.
- The anchors of the former sections (`/actors#actor-clerk`) are not redirected: they land on the entry of the actor
  in the index, which holds the same id.
