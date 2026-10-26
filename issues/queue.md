# Issue queue

Open issues in the order to work on them: the first is the next one. Read by `just next-issue`;
`just close-issue` removes a line, `skill:create-issue` inserts one at its rank, `skill:prioritize-issues`
re-ranks the whole list. Line format: `- [<file stem>](<file stem>.md) - <why here>`.

- [reconciliation_judge_sees_whole_sections](reconciliation_judge_sees_whole_sections.md) - right after `reconciliation_recall_on_real_docs`: most judge answers on linkding rest on sections cut at 1,500 chars, and the wider match of REC-2 sends it more of them
- [rules_put_back_what_was_set_aside](rules_put_back_what_was_set_aside.md) - moderate: safety net for the false yes of the question on features about tests (a wrong yes hides every rule of a feature), after `docs_reconciliation_debt_report` which reads the rules and the report that lists them; the rules pass is off by default, so not before
- [shorter_use_case_answers](shorter_use_case_answers.md) - lever B of the closed `use_cases_cost_per_call`; the input side (`one_code_reading_for_use_cases_and_rules`) is closed without changing the prompts
- [second_application_in_benchmark](second_application_in_benchmark.md) - after `benchmark_passes_on_fixed_inputs`, before the issues that compare settings (`measure_model_split_per_pass`, `settle_experimental_settings`): a gain on one application is not yet a gain
- [measure_model_split_per_pass](measure_model_split_per_pass.md) - after the pivot: it removes and changes passes, which would make an earlier split measure stale
- [settle_experimental_settings](settle_experimental_settings.md) - after the measures it needs (`benchmark_passes_on_fixed_inputs`, the rules issues); fewer options before the refactoring
- [parsed_references_for_ranking_and_slices](parsed_references_for_ranking_and_slices.md) - split off `tree_sitter_structure` step 3 (2026-10-11): gain unmeasured, starts with a no-LLM comparison of slices, after the passes it reads have settled
- [brief_injection_experiments](brief_injection_experiments.md) - only once the pipeline stopped moving (behaviour issue) and with a steadier measure
- [surface_domains_default_decision](surface_domains_default_decision.md) - parked (user, 2026-10-10): an optimization; the default stays `files` until the cost of more benchmark runs is acceptable
- [single_pipeline_path](single_pipeline_path.md) - right after `surface_domains_default_decision`, which it applies
- [pass_runner_and_run_context](pass_runner_and_run_context.md) - after `single_pipeline_path`: do not refactor code about to be removed
- [contexts_residual_size_and_names](contexts_residual_size_and_names.md) - low: follow-up of the closed split by family (2026-10-14): one context still over 150 files, action names taken for families, the names unread; nothing before it depends on it, and its spread measure is cheaper after `sampling_temperature_for_steadier_runs`
- [rules_plumbing_and_screen_noise](rules_plumbing_and_screen_noise.md) - low (user, 2026-10-23): fine tuning of rules that are already usable; each step gained less than the one before, so after `contexts_residual_size_and_names` and well behind the structural work
- [git_log_follow_renames](git_log_follow_renames.md) - not a priority (user, 2026-10-09): small MCP improvement, do it when nothing more valuable is left
- [smoke_warnings_business_paths_sources_and_empty_use_cases](smoke_warnings_business_paths_sources_and_empty_use_cases.md) - priority low; remeasure after the big changes
- [rerun_rewrites_files_with_a_timestamp](rerun_rewrites_files_with_a_timestamp.md) - priority low (user, 2026-10-16): the rerun of the same smoke writes 3 files for a `generated_at` stamp; cosmetic for the smoke criterion
- [merge_form_screens_into_their_use_case](merge_form_screens_into_their_use_case.md) - on hold (user, 2026-10-15): the measure found the real pairs at about 2 to 5% of a 52 use cases run; retake it on a complete run before deciding (see the issue)
