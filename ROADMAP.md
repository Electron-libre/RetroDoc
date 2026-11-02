# Roadmap

The planned work of the project, grouped into milestones, in the order they are meant to land: the user orders them.
A milestone becomes issues once it is shaped (`skill:create-issue`); the `priority` and `depends_on` of each issue file order them within a milestone (`just next-issue`).
States: `idea`, `shaped`, `in issues`, `done`; `just release` adds `- Released: v<version>` to a `done` one. A milestone carries no date unless the project gives it one.

The milestones up to "A large application, one bounded context at a time" are the phases 1 to 11 of the former
`PLAN.md`, in their order; the dated findings behind them are in `docs/findings.md` and the closed issues in
`issues/done/`.

## Foundation

- Goal: a CLI skeleton, the `retrodoc.toml` config (OpenRouter key and model, ignored paths) and the basic ingestion
  (walker, git history, existing Markdown docs).
- Why: every later pass reads the repository through it.
- State: done
- Released: v0.1.0

## Repo map

- Goal: bottom-up LLM summaries of files and directories, enriched with the git history.
- Why: the first input of the domain clustering (ADR 0004); removed with the `files` path (ADR 0035).
- State: done
- Released: v0.1.0

## Domains

- Goal: the domains clustered from the repository, with a coverage check.
- Why: the top level of the functional docs; the sub-domains were removed with the `files` path (ADR 0035).
- State: done
- Released: v0.1.0

## Features, use cases and diagrams

- Goal: features per domain, use cases per feature with steps and actors, a Mermaid diagram per use case.
- Why: the body of the functional documentation.
- State: done
- Released: v0.1.0

## Confidence and debt report

- Goal: a confidence score per generated section and a documentation debt report.
- Why: a reader must know how far the code supports each claim.
- State: done
- Released: v0.1.0

## Writing and CLI ergonomics

- Goal: dry-run with diff preview, idempotent writing, incremental re-run.
- Why: a rerun on an unchanged repository must change nothing and call no LLM.
- State: done
- Released: v0.1.0

## Business-level documentation

- Goal: docs that read as functional documentation, not as a paraphrase of the code, built on a surface extraction
  (file roles, entities and glossary, entry points and outputs, business actors, narrative, business-language score).
- Why: the first smoke tests gave mostly technical docs (the risks of `PRODUCT.md`, `docs/findings.md` §2); its open work moved to the top-down
  redesign.
- State: done
- Released: v0.1.0
- Issues:
  - [narrative_checked_against_cited_code](issues/done/narrative_checked_against_cited_code.md)

## Scalability and cost control

- Goal: a run on a large repository within reach, including on a local LLM: progress and heartbeat, batching,
  optional or sampled confidence, concurrency, file budget, resumable runs, token recap per pass.
- Why: the run time and the cost did not scale past a few hundred files (`docs/findings.md` §3); the open part is the
  milestone "Pre-run cost estimate".
- State: done
- Released: v0.1.0

## MCP server over the generated docs

- Goal: a read-only, LLM-free MCP server and `retrodoc search` (BM25) over the generated and collected docs, with
  freshness warnings and `read_source` / `git_log` limited to the cited files (ADR 0016).
- Why: the first consumer of "ask the documentation" (`docs/findings.md` §4) is an external agent.
- State: done
- Released: v0.1.0
- Issues:
  - [mcp_server](issues/done/mcp_server.md)

## Top-down redesign

- Goal: understand the product from the non-code evidence first, cover behaviours instead of files, business rules in
  the model, code structure from tree-sitter, structured outputs with a model per pass, reconciliation with the
  existing docs (ADRs 0019 to 0023, `docs/findings.md` §6), then the follow-ups of the review of 2026-10-12 (`docs/findings.md` §7): steadier
  measures, one pipeline path, the pass runner.
- Why: the pipeline climbed from the code to the business and its cost grew with the file count.
- State: done
- Released: v0.1.0
- Issues:
  - [quality_benchmark](issues/done/quality_benchmark.md)
  - [product_brief](issues/done/product_brief.md)
  - [locate_business_files](issues/done/locate_business_files.md)
  - [structured_llm_outputs](issues/done/structured_llm_outputs.md)
  - [model_per_pass](issues/done/model_per_pass.md)
  - [behaviour_coverage_domains](issues/done/behaviour_coverage_domains.md)
  - [tree_sitter_structure](issues/done/tree_sitter_structure.md)
  - [business_rules](issues/done/business_rules.md)
  - [docs_reconciliation_debt_report](issues/done/docs_reconciliation_debt_report.md)
  - [sampling_temperature_for_steadier_runs](issues/done/sampling_temperature_for_steadier_runs.md)
  - [benchmark_passes_on_fixed_inputs](issues/done/benchmark_passes_on_fixed_inputs.md)
  - [settle_experimental_settings](issues/done/settle_experimental_settings.md)
  - [surface_domains_default_decision](issues/done/surface_domains_default_decision.md)
  - [single_pipeline_path](issues/done/single_pipeline_path.md)
  - [pass_runner_and_run_context](issues/done/pass_runner_and_run_context.md)

## Language and framework agnostic

- Goal: every pass works on a repository of a stack the code knows nothing of; the stack tables and readers are
  accelerators with a generic fallback, and cover the reference set of stacks (ADR 0036).
- Why: the stack knowledge grew from the measured repositories only, so the tables drifted towards their stacks.
- State: done
- Released: v0.2.0
- Issues:
  - [stack_tables_cover_the_reference_set](issues/done/stack_tables_cover_the_reference_set.md)
  - [framework_fixtures_through_generate](issues/done/framework_fixtures_through_generate.md)
  - [tree_sitter_reference_languages](issues/done/tree_sitter_reference_languages.md)
  - [stack_tables_nested_layouts](issues/done/stack_tables_nested_layouts.md)
  - [roles_second_pass_and_promoted_sources](issues/done/roles_second_pass_and_promoted_sources.md)
  - [view_template_labels](issues/done/view_template_labels.md)
  - [template_extensions_never_promoted](issues/done/template_extensions_never_promoted.md)
  - [test_layouts_singular_and_variants](issues/done/test_layouts_singular_and_variants.md)
  - [built_in_extension_templates_read_as_source](issues/done/built_in_extension_templates_read_as_source.md)

## A large application, one bounded context at a time

- Goal: an application too large for one run documented in resumable, complementary runs, one bounded context at a
  time (`retrodoc contexts`, `domains_from = "contexts"`, ADR 0029, `docs/findings.md` §8).
- Why: one run on a 1,676-file application stopped at 60 files; delivered and run on 8 of its 35 contexts, the quality
  follow-ups and the contexts left are open.
- State: done
- Released: v0.3.0
- Issues:
  - [validate_on_large_application](issues/done/validate_on_large_application.md)
  - [business_files_cap_on_large_application](issues/done/business_files_cap_on_large_application.md)
  - [split_contexts_by_family_not_by_layer](issues/done/split_contexts_by_family_not_by_layer.md)
  - [use_cases_record_their_entry_points](issues/done/use_cases_record_their_entry_points.md)
  - [technical_and_back_office_use_cases](issues/done/technical_and_back_office_use_cases.md)
  - [contexts_residual_size_and_names](issues/done/contexts_residual_size_and_names.md)
  - [merge_form_screens_into_their_use_case](issues/done/merge_form_screens_into_their_use_case.md)
  - [actors_read_long_authorization_files](issues/done/actors_read_long_authorization_files.md)

## Business rules quality

- Goal: rules without duplicates, plumbing or screen behaviour, measured on a replay that gets the evidence
  `generate` gives.
- Why: the rules are usable but still carry noise (`docs/findings.md` §6, follow-up of step 6).
- State: done
- Released: v0.4.0
- Issues:
  - [rules_deduplication_and_noise](issues/done/rules_deduplication_and_noise.md)
  - [rules_replay_without_evidence](issues/done/rules_replay_without_evidence.md)
  - [rules_plumbing_and_screen_noise](issues/done/rules_plumbing_and_screen_noise.md)
  - [rules_replay_without_put_back](issues/done/rules_replay_without_put_back.md)
  - [rules_judge_prompt_feature_on_one_line](issues/done/rules_judge_prompt_feature_on_one_line.md)
  - [one_line_at_the_source](issues/done/one_line_at_the_source.md)

## Browse the docs in a web interface

- Goal: `retrodoc ui` serves the generated docs to a human in a browser: the tree of domains, features and use cases,
  each page with its steps, actors, narrative, rules, diagram, confidence and freshness, the links to the existing
  docs, the search, the glossary and the code the docs cite (ADR 0039).
- Why: the results of a run are read file by file in Markdown; whoever takes an application over needs a structured
  way to browse them, and the next milestones of the interface build on it.
- State: done
- Released: v0.5.0
- Issues:
  - [web_ui_server_and_domains](issues/done/web_ui_server_and_domains.md)
  - [web_ui_feature_and_use_case_pages](issues/done/web_ui_feature_and_use_case_pages.md)
  - [web_ui_search_glossary_and_collected_docs](issues/done/web_ui_search_glossary_and_collected_docs.md)
  - [web_ui_cited_code_and_files_to_docs](issues/done/web_ui_cited_code_and_files_to_docs.md)
  - [web_ui_rules_and_actors](issues/done/web_ui_rules_and_actors.md)

## Web interface for a first-time reader

- Goal: the web interface names the application it documents, opens on a home page that tells what the application
  does and where to start, draws the docs as a map to explore, and wears the Meta Lambda colours and credit.
- Why: on a saved run (2026-11-02) a reader could not tell which application the docs were about nor where to start
  reading.
- State: in issues
- Issues:
  - [web_ui_application_name](issues/done/web_ui_application_name.md)
  - [web_ui_guided_entry](issues/web_ui_guided_entry.md)
  - [web_ui_meta_lambda_branding](issues/web_ui_meta_lambda_branding.md)
  - [web_ui_visual_map_of_the_docs](issues/web_ui_visual_map_of_the_docs.md)

## Docs in the reader's language

- Goal: a `retrodoc.toml` setting chooses the language the generated docs are written in.
- Why: the model writes in the language it picks (English on the saved runs); a team that reads another language
  cannot take the docs as they are.
- State: in issues
- Issues:
  - [generated_docs_language](issues/generated_docs_language.md)

## Explore the documentation debt and audit it

- Goal: in the web interface, one filterable view of the debt (weak and unscored sections, unbacked narratives, set
  apart use cases, existing docs contradicted or missing, files and entry points no doc covers, stale sections) and
  an audit dashboard (overall confidence, business language, coverage, stale share, a heatmap domain by indicator,
  cost per pass, progress of the bounded contexts), with a priority score that crosses the debt with the git churn
  and the entry points.
- Why: the debt report is one long page; deciding where to improve the docs needs to see where the debt
  concentrates and what to fix first.
- State: idea

## Run history

- Goal: a snapshot of the debt report and the figures at each `generate`, the trends across runs in the dashboard,
  and the difference between two runs (added, removed, degraded sections).
- Why: the report keeps the last state only, so nobody can tell whether the docs get better.
- State: idea

## Human review of the docs

- Goal: mark a section validated, wrong or to review, with a comment, from the web interface; the reviews are kept
  in `.retrodoc/`, shown in the debt report, the dashboard and the MCP answers.
- Why: the confidence score is the model's view; a reader must know what a human has checked.
- State: idea

## Edit the docs

- Goal: complete or correct a section from the web interface; a human change is kept by the next `generate` and
  flagged when the code it rests on changes.
- Why: the generated docs are a start; the team must be able to own them without losing their work at each run.
- State: idea

## Pre-run cost estimate

- Goal: before a run, an estimate of the calls and tokens of the whole pipeline, extrapolated from the measured ratios
  of `usage.json`.
- Why: the token recap gives the cost after the run only (step 2 of the token accounting, `docs/findings.md` §3).
- State: idea

## Ask the documentation

- Goal: an internal `retrodoc ask` / `chat` agent answering from the generated docs, the collected docs, the git
  history and the code, with citations, confidence and freshness warnings (`docs/findings.md` §4, steps 2 to 4).
- Why: the MCP server serves an external agent; a user without one has no way to question the docs.
- State: in issues
- Issues:
  - [search_relevance_threshold](issues/done/search_relevance_threshold.md)
