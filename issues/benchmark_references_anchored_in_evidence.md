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
* Today: linkding 30 features (188 lines), Lobsters 38 features (274 lines).
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
