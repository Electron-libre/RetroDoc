# Issue queue

Open issues in the order to work on them: the first is the next one. Read by `just next-issue`;
`just close-issue` removes a line, `skill:create-issue` inserts one at its rank, `skill:prioritize-issues`
re-ranks the whole list. Line format: `- [<file stem>](<file stem>.md) - <why here>`.

- [business_files_cap_on_large_application](business_files_cap_on_large_application.md) - found by `validate_on_large_application` (2026-10-12), widened by the user: a large application in several complementary runs, one bounded context at a time, which removes the cap of 60 business files; first, since the validation run is paused until it is done
- [validate_on_large_application](validate_on_large_application.md) - project review of 2026-10-12: it says whether the redesign reaches the goal on the real target, and feeds `surface_domains_default_decision`; paused (user, 2026-10-12) after its reading stage, resumes once `business_files_cap_on_large_application` is done
- [technical_and_back_office_use_cases](technical_and_back_office_use_cases.md) - found by the control pass of `validate_on_large_application` (2026-10-13): 30% of the use cases of the first real run are technical helpers, back-office tools or system behaviours with a human actor; the first thing a reader of the docs meets
- [split_contexts_by_family_not_by_layer](split_contexts_by_family_not_by_layer.md) - same run: the largest context mixes subjects because the workflow folder was handed over by layer; it decides what the next contexts look like, so before documenting the 23 left
- [use_cases_record_their_entry_points](use_cases_record_their_entry_points.md) - same run: the share of behaviours the use cases reach cannot be measured (the field is empty in 112 of 161); needed to judge the two issues above and `shorter_use_case_answers`
- [sampling_temperature_for_steadier_runs](sampling_temperature_for_steadier_runs.md) - small, and before every issue that measures: a smaller spread makes each later comparison cheaper
- [rules_deduplication_and_noise](rules_deduplication_and_noise.md) - follow-up of business_rules (smoke test, 2026-10-12): the rules must be stated once and be business ones before the docs are reconciled with them
- [benchmark_passes_on_fixed_inputs](benchmark_passes_on_fixed_inputs.md) - after the rules work in progress, before the issues that compare settings (`measure_model_split_per_pass`, `brief_injection_experiments`, `settle_experimental_settings`), which need this measure
- [one_code_reading_for_use_cases_and_rules](one_code_reading_for_use_cases_and_rules.md) - after `rules_deduplication_and_noise` (same pass) and the per-pass measure: the largest token saving, about 80% of a run's input is these two passes
- [docs_reconciliation_debt_report](docs_reconciliation_debt_report.md) - first PRODUCT.md feature, needs stable generated docs with rules
- [shorter_use_case_answers](shorter_use_case_answers.md) - lever B of the closed `use_cases_cost_per_call`; after the input side (`one_code_reading_for_use_cases_and_rules`) which changes the same prompts
- [measure_model_split_per_pass](measure_model_split_per_pass.md) - after the pivot: it removes and changes passes, which would make an earlier split measure stale
- [settle_experimental_settings](settle_experimental_settings.md) - after the measures it needs (`benchmark_passes_on_fixed_inputs`, the rules issues); fewer options before the refactoring
- [parsed_references_for_ranking_and_slices](parsed_references_for_ranking_and_slices.md) - split off `tree_sitter_structure` step 3 (2026-10-11): gain unmeasured, starts with a no-LLM comparison of slices, after the passes it reads have settled
- [brief_injection_experiments](brief_injection_experiments.md) - only once the pipeline stopped moving (behaviour issue) and with a steadier measure
- [surface_domains_default_decision](surface_domains_default_decision.md) - parked (user, 2026-10-10): an optimization; the default stays `files` until the cost of more benchmark runs is acceptable
- [single_pipeline_path](single_pipeline_path.md) - right after `surface_domains_default_decision`, which it applies
- [pass_runner_and_run_context](pass_runner_and_run_context.md) - after `single_pipeline_path`: do not refactor code about to be removed
- [git_log_follow_renames](git_log_follow_renames.md) - not a priority (user, 2026-10-09): small MCP improvement, do it when nothing more valuable is left
- [smoke_warnings_business_paths_sources_and_empty_use_cases](smoke_warnings_business_paths_sources_and_empty_use_cases.md) - priority low; remeasure after the big changes
