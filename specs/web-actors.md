---
id: WAC
status: accepted
adrs: [0039]
---

# Actors page of the web interface

## Intent

The actors page of `retrodoc ui`, `/actors`, tells whoever takes an application over who its actors are and, for one
actor, what it starts and takes part in, read top to bottom on an application of any size.

## Requirements

### Behaviour

- **WAC-1** (deterministic) The actors page must show an index of the actors, the human ones then the system ones under
  their own heading, each sorted by name ignoring case, each a link to its section followed by the number of use cases
  it starts and the number of the others it takes part in.
- **WAC-2** (deterministic) The actors page must show, after the index, a section per actor in the order of the index,
  headed by its name and the two numbers of WAC-1.
- **WAC-3** (deterministic) The section of an actor must list the use cases it starts (its primary actor) or takes part
  in (the actor of one of their steps), each a link to its page, grouped under their domain then their feature, each
  a link to its page, in the order of the run, the use cases it starts first in their feature and marked.
- **WAC-4** (deterministic) The section of an actor must list after its use cases the permission rules that apply to
  one of them, each a link to its item on the page of its feature.
- **WAC-5** (deterministic) The map of a domain (`/map/<domain>`, WEB-73) must link each of its actors to the section
  of that actor on the actors page.

### Contract

- **WAC-6** (deterministic) The actors must be grouped by their exact name, and be the primary actors of the use cases
  and the actors of their steps; a primary actor no step names is a human one, and a name the steps give both kinds is
  an actor of each kind whose primary use cases are started by the human one only.
- **WAC-7** (deterministic) The section of an actor must carry the id `actor-<slug>` for a human one and
  `system-<slug>` for a system one, the slug its name in lower case with each run of characters other than letters
  and digits (of any script) read as one `-`, none at either end (`actor` for a name with none left), and `-2`, `-3`… after
  an id an earlier actor of the index or a list of the index already took; the index lists the human actors under the id `human-actors` and
  the system ones under `system-actors`.
- **WAC-8** (deterministic) The actors page must not hold a table: no column per feature, nor per domain.

## Scenarios

- **WAC-H1** (happy; WAC-1, WAC-2, WAC-3, WAC-4) Given a run whose `Clerk` starts `pay-by-card` and takes a step of
  `refund` and of `ship`, when the reader opens `/actors`, then the index lists `Clerk` under the people with
  "starts 1" and "takes part in 2", and its section lists `pay-by-card` marked then `refund` under `Billing` › `Pay an
  invoice`, then `ship` under its own domain and feature, then the permission rules of the three.
- **WAC-H2** (happy; WAC-5, WAC-7) Given the map of a domain whose feature `Clerk` starts a use case of, when the
  reader follows `Clerk` on it, then they land on the section `#actor-clerk` of the actors page.
- (WAC-6, WAC-7) Given `Clerk` and `clerk`, then they are two actors, `#actor-clerk` then `#actor-clerk-2`.
- (WAC-6) Given a primary actor no step names, then it is a human actor; given a name the steps give both kinds, then
  it is listed under each, and as a primary actor it starts its use cases in the human section only.
- (WAC-1, WAC-3) Given an actor that both starts a use case and takes part in one of its steps, then the use case is
  listed once, marked, and counted among those it starts only.
- (WAC-3) Given a feature none of whose use cases names the actor, then the section of the actor does not show it,
  nor a domain none of whose features it shows.
- (WAC-3) Given a feature whose domain the run does not list, then it is listed under the slug of that domain, with no
  link.
- (WAC-6) Given a use case whose feature is not in the run, then its actors are not listed for it, nor at all when it
  is the only use case that names them.
- (WAC-4) Given an actor none of whose use cases a permission rule applies to, then its section lists no rule.
- (WAC-7) Given the name `Payment gateway (Stripe)`, then its id is `system-payment-gateway-stripe`; given `???`,
  then it is `actor-actor`; given `Équipe support`, then it is `actor-équipe-support`.
- (WAC-7) Given a system actor named `Actors`, then its id is `system-actors-2`, the list of the systems holding
  `system-actors`.
- (WAC-1) Given a run with no use case that names an actor, then the page says no actor was found.
- (WAC-1) Given a repository with no run yet, then the page says to run `retrodoc generate`.

## Non-goals

- No page per actor (`/actors/<slug>`): the section of an actor is the place the map links to; the search does not
  list the actors.
- No overview of the actors by domain: the index gives the numbers of each actor, its section the detail.
- The generic system actors (a "System" that takes part in almost every use case) are not recognised: they are
  listed with the other system actors, under their heading after the people.
- The page does not merge the spellings of an actor nor maps them to the actors list (`actors.yaml`).
- The use cases of an actor are not paged nor folded: a long section is read by scrolling.
