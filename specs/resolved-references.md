---
id: RES
status: draft
adrs: [0022, 0027, 0036]
---

# Resolved references

## Intent

The slice of an entry point (the code a use case is derived from) follows what each file really uses: its imports,
its parent classes and the calls whose receiver the code makes known, resolved to the files that define them, with
no external tool; what the resolution cannot see keeps today's lexical follow.

## Requirements

### Behaviour

- **RES-1** (deterministic) For a Python or Ruby file of the repository, the resolution must give the files of the
  repository that the file imports, inherits from and calls through a known receiver.
- **RES-2** (deterministic) A slice must follow, from a file with at least one resolved reference, its resolved
  references only, and, from a file with none (a language with no resolver, a file the build cannot parse, a file
  linked to others by strings only), the lexical follow.

### Contract

- **RES-3** (deterministic) A Python import must resolve to the file of the module it names: absolute
  (`import shop.orders`, `from shop.orders import Order`, as a path from the repository root or from a directory
  holding a top-level package) and relative (`from .orders import Order`, `from ..models import Order`); a name
  imported from a package resolves to the module of that name when there is one (`from shop import orders` to
  `shop/orders.py`), else to the package's `__init__.py`.
- **RES-4** (deterministic) A Python name bound by an import (a class, a function, a module), when the file uses it
  (a call, an attribute read, a parent class, a decorator), must resolve to the file the import resolves to.
- **RES-5** (deterministic) A Ruby `require_relative` must resolve to the file it names, and a `require` of a path
  to the file under one of the load roots of the repository (`lib/`, `app/*/`, the root) that matches it.
- **RES-6** (deterministic) A Ruby constant (`Order`, `Shop::Order`) must resolve to the files defining the class or
  module of that qualified name, looked up from the innermost namespace of the reference outwards, then at the top
  level.
- **RES-7** (deterministic) A call on `self` or with no receiver (Python `self.f()`, `cls.f()`, Ruby `f`,
  `self.f`), and a call on a class constant (`Order.find`), must resolve to the file defining that method in the
  class, else in its ancestors (parent classes, then Ruby's included and extended modules) that the resolution
  knows, nearest first.
- **RES-8** (deterministic) A call on a receiver whose class the resolution does not know (a local, a parameter, the
  result of another call) must not resolve to any file: no guess by the method's bare name.
- **RES-9** (deterministic) A reference that resolves only to the referencing file itself or outside the
  repository's source files (the standard library, a gem, a package) must not give a file.
- **RES-10** (deterministic) A file of a language with no resolver in the build (its grammar feature off, or a
  language other than Python and Ruby) must get no resolved reference, so that the slice keeps the lexical follow.
- **RES-11** (deterministic) The slices of the passes (use cases, contexts, the reach of the entry points, the
  behaviour coverage) must keep their depth and file budget, and rank within a hop the files a file references most
  first, as the lexical follow does.

## Scenarios

- **RES-H1** (happy; RES-1, RES-2, RES-3, RES-4) Given a Django view `shop/views.py` that does
  `from .models import Order` and `from shop import queries`, then calls `queries.open_orders()` and
  `Order.objects.filter(...)`, when its slice is cut, then it holds `shop/queries.py` and `shop/models.py`, and no
  file merely named like a method it calls (`filter.py`).
- **RES-H2** (happy; RES-1, RES-2, RES-6, RES-7, RES-8) Given a Rails controller
  `app/controllers/stories_controller.rb` with `class StoriesController < ApplicationController`, a call
  `require_login` defined in `ApplicationController`, `Story.find(params[:id])` and `story.save`, when its slice is
  cut, then it holds `app/controllers/application_controller.rb` and `app/models/story.rb`, and no file defining
  another `save` method.
- **RES-H3** (happy; RES-2, RES-10) Given a `.rake` task and a Django management command calling another one by
  `call_command("sync")`, when their slices are cut, then each is the lexical slice of today.
- (RES-3) Given `from ..models import Order` in `shop/api/views.py`, then it resolves to `shop/models.py`; given
  `from shop import orders` with both `shop/orders.py` and `shop/__init__.py`, then to `shop/orders.py`; given
  `import json`, then to no file.
- (RES-3) Given a repository whose package sits under `src/` (`src/shop/orders.py`), then `import shop.orders`
  resolves to it.
- (RES-4) Given `from shop.models import Order as O` and `class Refund(O):`, then the parent class resolves to
  `shop/models.py`.
- (RES-5) Given `require_relative "../lib/pricing"` in `app/services/checkout.rb`, then it resolves to
  `lib/pricing.rb`; given `require "json"`, then to no file.
- (RES-6) Given `module Shop; class Checkout; Order.new; end; end` and both `Shop::Order` (`app/models/shop/order.rb`)
  and a top-level `Order` (`app/models/order.rb`), then `Order` resolves to `app/models/shop/order.rb`; with no
  `Shop::Order`, to `app/models/order.rb`.
- (RES-6) Given a class reopened in two files (`class Order` in `app/models/order.rb` and
  `lib/ext/order.rb`), then the constant resolves to both.
- (RES-7) Given `class Admin < User` with `def promote; save_role; end`, `save_role` defined in `User` and in
  an unrelated `Group`, then the call resolves to `User`'s file only; given a module `Auditable` defining
  `audit` and `include Auditable` in `User`, then `audit` called in `Admin` resolves to `Auditable`'s file.
- (RES-7) Given `self.helper()` in a Python method of a class that defines `helper` itself, then it resolves to the
  file itself, so no file (RES-9).
- (RES-8) Given `bookmark = get_bookmark(id)` then `bookmark.archive()`, and `archive` defined in three files, then
  `archive` resolves to no file.
- (RES-10) Given a Go file of the same repository, then it has no resolved reference and its slice is the lexical
  one.
- (RES-11) Given a file referencing `shop/models.py` three times and `shop/queries.py` once, then the slice ranks
  `shop/models.py` first; given more resolved files than the budget, then the slice stops at the budget.

## Non-goals

- Resolving other languages: each gets its resolver behind its grammar feature in a later change; until then it
  keeps the lexical follow (RES-10).
- Framework routing or conventions (Rails autoloading paths, Django URL strings, `call_command` names): adapters
  per framework are rejected (ADR 0036); a Ruby constant is resolved by its definitions, not by the file layout.
- Type inference: the class of a local, a parameter or a returned value is not inferred (RES-8).
- Dynamic dispatch (`send`, `getattr`, `method_missing`, metaprogrammed methods), monkey patching beyond reopened
  classes (RES-6).
- The ranking of files by importance (`ranking.rs`) and the placement of contexts and routes, which keep their
  lexical counts.
- The same slices as codegraph: its edges are a reference to compare with, not a target.
