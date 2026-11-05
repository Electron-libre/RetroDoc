# Roadmap

The planned work of the project, grouped into milestones, in the order they are meant to land: the user orders them.
A milestone becomes issues once it is shaped (`skill:create-issue`); the `priority` and `depends_on` of each issue file order them within a milestone (`cardinal run next-issue`).
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
- Why: one run on a 1,676-file application stopped at 60 files; delivered and run on 8 of its 35 contexts. Running
  the contexts left is use of the tool, not work on it; the spread of the map from run to run is in the milestone
  "Measured quality at the current code".
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
- State: done
- Released: v0.6.0
- Issues:
  - [web_ui_application_name](issues/done/web_ui_application_name.md)
  - [web_ui_guided_entry](issues/done/web_ui_guided_entry.md)
  - [web_ui_meta_lambda_branding](issues/done/web_ui_meta_lambda_branding.md)
  - [web_ui_visual_map_of_the_docs](issues/done/web_ui_visual_map_of_the_docs.md)

## Docs in the reader's language

- Goal: a `retrodoc.toml` setting chooses the language the generated docs are written in.
- Why: the model writes in the language it picks (English on the saved runs); a team that reads another language
  cannot take the docs as they are.
- State: done
- Released: v0.6.0
- Issues:
  - [generated_docs_language](issues/done/generated_docs_language.md)
  - [localized_render_and_web_text](issues/done/localized_render_and_web_text.md)
  - [language_native_names_and_regional_tags](issues/done/language_native_names_and_regional_tags.md)
  - [reconcile_across_languages](issues/done/reconcile_across_languages.md)
  - [reconcile_excerpt_by_identifiers](issues/done/reconcile_excerpt_by_identifiers.md)
  - [render_removes_pages_of_renamed_items](issues/done/render_removes_pages_of_renamed_items.md)

## Release and code hygiene

- Goal: `retrodoc --version` gives the released version, and the measuring tools (benchmark, replays, `*_measure`
  examples) live beside the pipeline instead of inside it.
- Why: the audit of 2026-11-03 found the workspace still at `0.1.0` after the tag `v0.6.0`, and about 11k of the 44k
  lines of `retrodoc-pipeline` are measuring tools.
- State: done
- Released: v0.7.0
- Issues:
  - [release_bumps_the_workspace_version](issues/done/release_bumps_the_workspace_version.md)
  - [measure_tooling_out_of_the_pipeline_crate](issues/done/measure_tooling_out_of_the_pipeline_crate.md)
  - [delete_one_off_measuring_examples](issues/done/delete_one_off_measuring_examples.md)
  - [delete_confidence_and_baseline_examples](issues/done/delete_confidence_and_baseline_examples.md)

## Measured quality at the current code

- Goal: a benchmark baseline at the current code on the two benchmark applications, a judge whose feature precision
  can be believed, references a human has read, a feature precision raised on applications, and a steadier map of
  bounded contexts.
- Why: the last whole benchmark predates the docs language, the narrative check and the new rules judgement
  (2026-10-28); feature precision is the weakest figure (47% judged on Lobsters on the baseline of
  2026-11-03, within 4 points of a hand count; earlier audits found the judge overstating it by 12 to 33); three
  maps of the same large application gave 34, 40 and 44 contexts. The web milestones below show these docs to
  readers: their quality comes first.
- State: done
- Released: v0.7.0
- Issues:
  - [benchmark_baseline_at_head](issues/done/benchmark_baseline_at_head.md)
  - [benchmark_references_anchored_in_evidence](issues/done/benchmark_references_anchored_in_evidence.md)
  - [judge_overstates_feature_precision](issues/done/judge_overstates_feature_precision.md)
  - [feature_precision_on_applications](issues/done/feature_precision_on_applications.md)
  - [technical_feature_put_back](issues/done/technical_feature_put_back.md)
  - [domain_precision_after_technical_set_aside](issues/done/domain_precision_after_technical_set_aside.md)
  - [judge_offer_hides_second_pairs](issues/done/judge_offer_hides_second_pairs.md)
  - [context_map_spread_between_runs](issues/done/context_map_spread_between_runs.md)
  - [context_map_support_and_catch_alls](issues/done/context_map_support_and_catch_alls.md)

## A local model, measured

- Goal: one complete benchmark run of an application on the local model, with its figures and duration next to the
  hosted baseline, or the principle of `PRODUCT.md` that a small local model is a valid target amended.
- Why: the only complete local attempt on linkding (2026-10-27) failed on the 120 s timeout; the loopback default is
  900 s since, and no local run has been made again.
- State: in issues
- Issues:
  - [complete_local_benchmark_run](issues/complete_local_benchmark_run.md)

## Read by someone taking an application over

- Goal: the generated docs of an application read by a person who takes it over (developer, product owner), with
  what they found right, wrong, missing and useless, written as dated findings.
- Why: the users of `PRODUCT.md` are assumed: only the author has read generated docs so far, and the benchmark
  measures likeness to a reference, not use.
- State: idea
- On hold: deferred by the user (2026-11-04).

## Explore the documentation debt and audit it

- Goal: in the web interface, one filterable view of the debt (weak and unscored sections, unbacked narratives, set
  apart use cases, existing docs contradicted or missing, files and entry points no doc covers, stale sections) and
  an audit dashboard (overall confidence, business language, coverage, stale share, a heatmap domain by indicator,
  cost per pass, progress of the bounded contexts), with a priority score that crosses the debt with the git churn
  and the entry points.
- Why: the debt report is one long page; deciding where to improve the docs needs to see where the debt
  concentrates and what to fix first.
- State: done
- Released: v0.7.0
- Issues:
  - [web_debt_view_of_the_docs](issues/done/web_debt_view_of_the_docs.md)
  - [web_debt_view_of_the_code](issues/done/web_debt_view_of_the_code.md)
  - [web_audit_dashboard](issues/done/web_audit_dashboard.md)
  - [debt_priority_score](issues/done/debt_priority_score.md)
  - [debt_priority_tuning](issues/done/debt_priority_tuning.md)

## Run history

- Goal: a snapshot of the debt report and the figures at each `generate`, the trends across runs in the dashboard,
  and the difference between two runs (added, removed, degraded sections).
- Why: the report keeps the last state only, so nobody can tell whether the docs get better.
- State: done
- Released: v0.8.0
- Issues:
  - [run_snapshot_at_generate](issues/done/run_snapshot_at_generate.md)
  - [web_run_trends](issues/done/web_run_trends.md)
  - [run_diff](issues/done/run_diff.md)

## Human review of the docs

- Goal: mark a section validated, wrong or to review, with a comment, from the web interface; the reviews are kept
  in `.retrodoc/`, shown in the debt report, the dashboard and the MCP answers.
- Why: the confidence score is the model's view; a reader must know what a human has checked.
- State: done
- Released: v0.9.0
- Issues:
  - [review_store](issues/done/review_store.md)
  - [web_review_form](issues/done/web_review_form.md)
  - [review_in_debt](issues/done/review_in_debt.md)
  - [review_in_mcp](issues/done/review_in_mcp.md)
  - [review_outdated](issues/done/review_outdated.md)

## Edit the docs

- Goal: complete or correct a section from the web interface; a human change is kept by the next `generate` and
  flagged when the code it rests on changes.
- Why: the generated docs are a start; the team must be able to own them without losing their work at each run.
- State: done
- Issues:
  - [edit_store](issues/done/edit_store.md)
  - [edits_in_read_model](issues/done/edits_in_read_model.md)
  - [web_edit_form](issues/done/web_edit_form.md)
  - [edit_outdated](issues/done/edit_outdated.md)
  - [edit_mark_in_markdown](issues/done/edit_mark_in_markdown.md)

## Pre-run cost estimate

- Goal: before a run, an estimate of the calls and tokens of the whole pipeline, extrapolated from the measured ratios
  of `usage.json`.
- Why: the token recap gives the cost after the run only (step 2 of the token accounting, `docs/findings.md` §3).
- State: in issues
- Issues:
  - [pass_ratios_from_usage_history](issues/done/pass_ratios_from_usage_history.md)
  - [pre_run_estimate](issues/done/pre_run_estimate.md)
  - [estimate_against_actual](issues/done/estimate_against_actual.md)
  - [estimate_business_read](issues/done/estimate_business_read.md)
  - [recap_passes_not_estimated](issues/recap_passes_not_estimated.md)
  - [estimate_passes_of_the_run_path](issues/estimate_passes_of_the_run_path.md)

## Ask the documentation

- Goal: an internal `retrodoc ask` / `chat` agent answering from the generated docs, the collected docs, the git
  history and the code, with citations, confidence and freshness warnings (`docs/findings.md` §4, steps 2 to 4).
- Why: the MCP server serves an external agent; a user without one has no way to question the docs.
- State: idea
- Issues:
  - [search_relevance_threshold](issues/done/search_relevance_threshold.md) (the MCP server and the search, its
    first form, are done)
