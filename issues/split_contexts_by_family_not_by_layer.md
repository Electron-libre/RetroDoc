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

# Approach
Measure first, with no LLM call, on the saved run: names shared by files across layers (a file stem, or a folder
named like a stem) in the orphan folders of the biggest parts.
1. Add secondary groups from the names that repeat across layers inside a folder (`<family>` in
   `processes/<family>/`, `<family>.rb`), and list them to the split call with their sample like the others.
2. Split again a part that stays over the limit, one more level, with a bound.
3. Check that a context's name fits its content (ask the call to name a part after most of its files).

# Resources
* ADR 0029, `crates/retrodoc-pipeline/src/contexts/` (`groups.rs`, `mod.rs` `split_context`)
* `issues/business_files_cap_on_large_application.md` (the map, step 3c, and the real runs)
* `crates/retrodoc-pipeline/examples/context_map_measure.rs`

# Hints
* A context already documented must stay as it is: changing the map redoes its slice. Offer the new split
  with `retrodoc contexts --force`, which asks again, edits included.
* The application is confidential: describe it generically.
