# Split a large context by business family, not by layer folder

# Goal
The split of a context of more than 150 files (ADR 0029) partitions its groups and its orphan folders. On a
framework whose workflows are laid out by layer (rules, tasks, processes, one folder each, every business
family in each), the folders cannot express a family, so the whole workflow layer lands in one part. Make the
split cut across layers by family, so that a context holds one subject.

# Findings (first real run on a large Rails application, 2026-10-13)
* The central context of the application came out as 7 to 8 parts. One of them (242 files, 5 entry point files)
  is named after drafting and templates and holds 15 features: form contracts and contract models, but also six
  families of approval and signing workflows (each one a kind of contract flow), the denial of a signature and
  11 notification use cases. The part named after the life cycle is small (37 files).
* The workflow folder has sub-folders by layer (rules, tasks, processes, state tables); a family is a file name
  (`<family>.rb`) and a folder of the same name under each layer (`processes/<family>/`). The mechanical groups
  are built around the names of the entry point files only, which are controllers; the workflow files have
  another role and join no group, so they are orphan folders, listed to the LLM by layer.
* A context left over 230 files after one level of splitting twice on that run.

# Findings of the work on it (2026-10-13)
* A first cause was a defect of the split, not the layout: the folders of the split context were all added as a
  fallback to its first part, so 43 folders were held by two contexts of the saved map and the first one won
  them (the whole workflow layer went to the "drafting" part whatever the split said). Fixed: a folder a part holds
  is no longer the fallback of the first one.
* The listing named a folder by sub-folder one level only; the families of the workflow layer are folders one
  level deeper (`processes/<family>/`, 148 of the 175 files). Fixed: a big folder is listed by its own sub-folders as
  deep as it stays big (5 levels at most), with a test.
* A part still over 150 files is split again (twice at most, at least three groups and folders to cut).
* On the copy of the real map (hosted `deepseek-chat`, 5 calls, 20k tokens in, $0.008): 35 contexts, the largest
  199 files; the families of the workflows are now grouped by subject (the multi-party flows together, the
  approval and signature flows together, the internal-document flow apart). The map still varies from a call to
  another (the support went from 14% to 38% of the files across the builds).

# Approach
Measure first, with no LLM call, on the saved run: names shared by files across layers (a file stem, or a folder
named like a stem) in the orphan folders of the biggest parts.
1. Add secondary groups from the names that repeat across layers inside a folder (`<family>` in
   `processes/<family>/`, `<family>.rb`), and list them to the split call with their sample like the others.
2. Split again a part that stays over the limit, one more level, with a bound.
3. Check that a context's name fits its content (ask the call to name a part after most of its files).

# Resources
* ADR 0029, `crates/retrodoc-pipeline/src/contexts/` (`groups.rs`, `mod.rs` `split_context`)
* `issues/done/business_files_cap_on_large_application.md` (the map, step 3c, and the real runs)
* `crates/retrodoc-pipeline/examples/context_map_measure.rs`

# Hints
* A context already documented must stay as it is: changing the map redoes its slice. Offer the new split
  with `retrodoc contexts --force`, which asks again, edits included.
* The application is confidential: describe it generically.

# Tracking

Approach step 2 (split a part again) was done before this plan; the deliverables cover steps 1 and 3.

1. [x] Family detection: a pure function in `groups.rs` that finds, among orphan files, the names repeated in
   at least 2 layer folders and covering at least 3 files (generic stems and layer names ignored). Unit test.
2. [x] Measure: an option of `context_map_measure` applies it to the orphan folders of the biggest parts of a
   saved run (families found, share of the orphans covered). Run on the saved run, figures in this issue.
3. [x] Use in the split: families computed in `split_context` only, listed to the call with their sample,
   kept in `contexts.yaml` as group rules. Tests with `FakeLlm`.
4. [x] Name fits content: an instruction in `SPLIT_SYSTEM_PROMPT` (name a part after most of its files), tested
   by its presence in the prompt.
5. [ ] Docs (`docs/crates/pipeline.md`, ADR 0029 consequences), `just check`, `just test-harness`.

Measure (deliverables 1 and 2, `context_map_measure --families`, saved run of the large Rails application):
* Whole application: 25 families over the 645 files in no group, covering 22% of them; noisy (folders such as
  `javascript`, `helper`, and action names such as `approve`).
* Under the workflow folder (`FAMILIES_UNDER=app/workflows`, 176 files in no group): 13 families covering 60% of
  the files; the 5 largest are real business families (the multi-party, consortium, market and internal-document
  flows), the others are action names (`validate`, `approve`, `restart`, `reissue`) that the folder of a
  `workflow_actions` layer repeats. Two iterations were needed: a name repeated in the folder of every family
  (`send`) is a step, and a folder that holds families is a layer unless it also names files.
* Known residue: an action name present in several layers is still taken for a family. The split call decides
  with the sample; to watch on the real run.

## Decisions

* Families are computed only when splitting a large context, never for the initial map.
* A family: a name in at least 2 distinct layer folders and at least 3 files; generic names ignored.
* The measure runs on a saved run of a large Rails application (copy in `/tmp`, not named here).
* The naming check is a prompt instruction, without a quality measure.
