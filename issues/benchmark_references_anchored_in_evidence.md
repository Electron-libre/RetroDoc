---
priority: medium
why: 'every recall and precision rests on these references; no one has checked them against a source independent of the agent that drafted them'
---
# Anchor the benchmark references in independent evidence

# Goal
The references of linkding and Lobsters (`reference.yaml`, `matches.yaml`, `entry-points.yaml`) were drafted by an
agent and checked against the code by a subagent. A wrong or missing reference item moves recall and precision as
much as a change of the pipeline. A human read is not the way out: the user does not know these applications and
would only rubber-stamp what the agent wrote (2026-11-03). Expected: each reference item cites evidence that a
script can check, the items no evidence backs or that an independent draft disputes are known, and the figures
carry how much they move when the disputed items are taken out.

# Findings
* `issues/done/quality_benchmark.md`: "a human spot check of `matches.yaml` is still due".
* `issues/done/benchmark_passes_on_fixed_inputs.md` (2026-10-24): "Checked by an agent, not by a human".
* Today: linkding 24 features (188 lines), Lobsters 31 features (274 lines).
* `PRODUCT.md`, risks: "the references are not all checked by a human".
* Human-written sources exist (2026-11-03), at the pinned commits:
  * linkding: the maintainers' user docs, `docs/src/content/docs/` (15 pages, about 1,850 lines: search, archiving,
    auto-tagging, api, browser-extension, admin, options, shortcuts, ...).
  * Lobsters: `app/views/about/about.html.erb` (429 lines, the page served at `lobste.rs/about`): guidelines, tags,
    invitation tree, ranking, flags, transparency, mailing list mode, private messages, search, story merging,
    hiding, suggested titles and tags, hats, feeds. It links meta stories that describe single features (story
    merging, suggested titles and tags, mailing list mode). `README.md` (149 lines) is about setup.
* The user's delivery_router reference is too small to say anything about the two others.

# Approach
1. **Evidence per item**: each feature (and use case where it differs) of a reference cites its evidence: a
   `doc` (path and heading in the repo's human-written docs, or the URL of a maintainer's page or meta story), or
   `code` (a route, controller, job or test). A check verifies that each cited path, heading or route exists at the
   pinned commit; URLs are recorded, not fetched.
2. **Blind second draft**: a subagent drafts a reference from the code and the human-written docs only, without
   seeing the current one. Items both drafts hold are agreed; each disagreement (missing, extra, split, merged) is
   settled by another subagent from the code, its decision and evidence recorded next to the item.
3. **Error bar**: re-score the kept runs with `--saved-judge` (no LLM) on the corrected references, then without the
   items backed by `code` only and disputed; record how much the figures move in the ledger and the findings.
4. **Provenance in the reference**: a field per reference (date, method) and per item (evidence kind), so the skill
   `quality-benchmark` can say which figures rest on doc-backed items. `PRODUCT.md` states the new risk in place of
   "not checked by a human".
5. **The user's part**: validate this rule once; no item-by-item review.

# Milestone
Measured quality at the current code

# Resources
* `benchmark/linkding/`, `benchmark/lobsters/`, skill `quality-benchmark`.
* `crates/retrodoc-bench/src/benchmark/reference.rs` (format of the reference).
* The cached clones at the pinned commits (skill `quality-benchmark`, "What `benchmark.rs run` does").

# Hints
* Lobsters has few human-written docs beyond the about page: there step 2 carries most of the weight; say so.
* The online about page may be newer than the pinned commit: cite the copy in the repo first.
* Changing a reference makes the earlier figures incomparable: re-score the kept runs in the same deliverable.
* The blind draft must not see `reference.yaml`, `matches.yaml` nor the generated docs, or it is not independent.

# Tracking

Plan: validated by Cedric Brancourt <cedric.brancourt@gmail.com> on 2026-11-03

## Understanding
Make each feature of the linkding and Lobsters references cite checkable evidence, find the disputed ones by a blind
second draft settled from the code, and give the error bar of the kept figures. Out of scope: evidence for domains,
use cases, rules, `matches.yaml` and `entry-points.yaml` (spec `benchmark-reference-evidence`, non-goals).

## Plan
1. [x] Format and check: `evidence`, `disputed` and `checked` in `reference.rs`, example `reference_evidence` (paths
   and headings checked in a tree, counts by backing, figures of saved reports without the disputed features);
   verified by tests on temp trees and reports; makes true BRE-1 to BRE-9. (autonomous)
2. [x] linkding: blind second draft by a subagent (code and maintainers' docs, not the reference nor the generated
   docs), diff, disagreements settled by another subagent from the code; evidence on each feature, `disputed`,
   `checked`; `matches.yaml` follows renames; check passes; `--saved-judge` re-score of the kept runs (f95ada6
   hidden, dd5cf43 baseline) and the error bar, in the ledger and this issue (`docs/findings.md` stops at
   2026-10-14); verified by the check and `just check`. (autonomous)
3. [x] Lobsters: the same, from `about.html.erb` at the pinned commit and the meta stories it links (`url`). (autonomous)
4. [x] Docs: skill `quality-benchmark` (which figures rest on doc-backed items, the check before a benchmark),
   `PRODUCT.md` risk, `docs/crates/bench.md`; verified by `update-docs` and the reviews. (autonomous)

## Decisions
- Mode: autonomous
- Base commit: 52f991bee49a5b92d66fe048f6300361986e5430

## Deliverable 1
- done: `evidence`, `disputed`, `checked` in the reference; module `evidence` and example `reference_evidence`.
- red: BRE-1, BRE-2, BRE-3, BRE-9: `no field evidence on type ReferenceFeature` (BRE-4 to BRE-8: tests written
  with the module, not run red first).
- not verified: the example on a reference with evidence (deliverables 2 and 3 write it).
- The finding "linkding 30 features, Lobsters 38" was wrong: 24 and 31.
- Code review: 2 findings, both fixed (a directory or a non-UTF-8 file read as missing; disputed count by name).

## Deliverable 2
- done: linkding reference settled (24 -> 26 features), evidence on each, 4 `disputed`, `checked`; `matches.yaml`:
  pairs of the dropped Save a PDF removed, metadata and bulk edit names moved to the split features, the generated
  names of the kept runs that hold the 3 added features paired by hand.
- Blind draft: 35 features in 9 domains. Settlement: 22 disagreements, 12 blind features merged into current
  ones, 2 dropped (keyboard shortcuts, task monitor: not capabilities), Save a PDF merged into the HTML snapshot,
  metadata and bulk edit split off, file attachments added, 9 descriptions fixed from the code (feeds, REST API
  scope, extension search, sign-in methods). Table: `~/retrodoc-runs/references-evidence/linkding-decisions.md`.
- `disputed` first marked 19 features, scope fixes included: without them precision fell 15 to 25 points because
  the core features (search, save, archive) left the reference. Narrowed to the 4 features one draft did not hold.
- Backing: 18 features by the user docs, 8 by code only (edit and delete, read later, archive, tag, merge tags,
  bundles, feeds, reader view).
- Figures (judged features recall/precision, mean of 3; `--saved-judge`, no LLM):

  | runs | before | after | after, without disputed |
  |---|---|---|---|
  | f95ada6 hidden | 89%/63% | 88%/71% | 88%/60% |
  | dafea18 hidden | 90%/57% | 91%/62% | 91%/57% |
  | dafea18 shown | 83%/56% | 87%/59% | 88%/57% |

  The precision gain comes from the hand pairs of the added features (generated metadata, bulk and asset features
  had nothing to match); the error bar on precision is about 10 points, on recall 1 to 2.
- The saved `judge.yaml` of 5 clones paired names with Save a PDF: retargeted to the HTML snapshot (`.orig` kept),
  else `--saved-judge` fails (BSC-3).
- red: none (data deliverable; the check of deliverable 1 passes on it).
- Code review: 1 finding, not fixed: `without_disputed` counts a generated name matched only to disputed features as unmatched; intended (BRE-8: the score of the reference without them, where that name is an extra).

## Deliverable 3
- done: Lobsters reference settled (31 -> 37 features), evidence on each, 9 `disputed`, `checked`; `matches.yaml`:
  pairs of Hide and save stories split by their words, flag review pairs moved, the generated names of the kept
  runs that hold the added and split features paired by hand.
- Online docs: `lobste.rs/about` is `app/views/about/about.html.erb` (cited at the pinned commit); 3 meta stories it
  links cited as `url` (story merging, suggested titles and tags, mailing list mode).
- Blind draft: 40 features in 11 domains, after merging 6 to fit the count the prompt suggested (a bias of the
  prompt: no target count for the settlement). Settlement: 23 disagreements; Hide and save stories split; story page,
  site statistics added; standing, merging, flag review split off; blind merges refused for comments, sign-up,
  moderation of stories and comments; the moderators list and banned IPs read with the log; new-user restrictions
  left in `rules`; 3 descriptions fixed from the code (no code moves a comment to another parent; mailing list mode
  shown "temporarily disabled" at this commit; tags and categories are for administrators). Table:
  `~/retrodoc-runs/references-evidence/lobsters-decisions.md`.
- Backing: 27 features by the about page or a maintainers' story, 10 by code only.
- Figures (judged features recall/precision, mean of 3; `--saved-judge`, no LLM):

  | runs | before | after | after, without disputed |
  |---|---|---|---|
  | f95ada6 hidden | 98%/65% | 94%/72% | 97%/60% |
  | dafea18 hidden | 96%/52% | 89%/55% | 95%/48% |
  | dafea18 shown | 98%/51% | 93%/55% | 98%/46% |

  Recall falls 4 to 7 points: some added features (merge stories, standing) no generated name holds; the error bar
  is about 5 points on recall and 7 to 12 on precision.
- red: none (data deliverable).
- Code review: no findings

## Deliverable 4
- done: skill `quality-benchmark` (the evidence, the check and its error bar, pairing added or split features);
  `PRODUCT.md` risk reworded; spec `benchmark-reference-evidence` accepted, its intent says what `disputed` marks.
- update-docs: `docs/crates/bench.md`, `docs/dev/commands.md` done in deliverable 1; map rows: n/a (an example and a
  module of the bench crate, covered by the crate page and the commands); no stale "checked by a human" left.
- red: none (docs).
- Code review: no findings
