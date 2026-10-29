# Issue queue

Open issues in the order to work on them: the first is the next one. Read by `just next-issue`;
`just close-issue` removes a line, `skill:create-issue` inserts one at its rank, `skill:prioritize-issues`
re-ranks the whole list. Line format: `- [<file stem>](<file stem>.md) - <why here>`.

- [reading_hash_ignores_the_prompt](reading_hash_ignores_the_prompt.md) - after `routes_file_entries_per_route`, whose prompt measure meets the trap first; a decision on the cache before more prompt changes are measured
- [entry_points_silent_empty_parts](entry_points_silent_empty_parts.md) - after `reading_hash_ignores_the_prompt`, whose cache decision its replay needs: routes lost with no warning in the default entry points, worth more than the follow-ups below
- [stack_tables_cover_the_reference_set](stack_tables_cover_the_reference_set.md) - after the entry points chain above (user, 2026-10-29: language and framework agnosticism, ADR 0036): small, makes the frequent stacks classified and briefed, and the fixtures below run on the full tables
- [framework_fixtures_through_generate](framework_fixtures_through_generate.md) - after `stack_tables_cover_the_reference_set`: proves the generic path and each frequent framework through `generate`, and shows what else breaks before the grammars
- [tree_sitter_reference_languages](tree_sitter_reference_languages.md) - after `framework_fixtures_through_generate`, whose fixtures then check each new grammar; the largest of the three, an accelerator over the generic path
- [contexts_residual_size_and_names](contexts_residual_size_and_names.md) - low: follow-up of the closed split by family (2026-10-14): one context still over 150 files, action names taken for families, the names unread; nothing before it depends on it, and its spread measure is cheaper after `sampling_temperature_for_steadier_runs`
- [rules_replay_without_evidence](rules_replay_without_evidence.md) - small, right before `rules_plumbing_and_screen_noise`: its rules measures should replay the pass with the evidence `generate` gives
- [rules_plumbing_and_screen_noise](rules_plumbing_and_screen_noise.md) - low (user, 2026-10-23): fine tuning of rules that are already usable; each step gained less than the one before, so after `contexts_residual_size_and_names` and well behind the structural work
- [smoke_run_keeps_previous_clone](smoke_run_keeps_previous_clone.md) - low (code review of the ledger issue, 2026-10-27): the skill's copy rule covers it until then; before `git_log_follow_renames`, it protects the saved runs the planned measures read
- [mcp_tools_answer_edge_cases](mcp_tools_answer_edge_cases.md) - low (high review of `split_mcp_tools`, 2026-10-29): edge cases of the MCP answers no run has shown yet; before `git_log_follow_renames`, same crate and also not a priority
- [render_markdown_edge_cases](render_markdown_edge_cases.md) - low (high review of `split_render_markdown`, 2026-10-29): edge cases of the render no run has shown yet; next to `mcp_tools_answer_edge_cases`, same kind, before `git_log_follow_renames`
- [openrouter_tests_shared_fixtures](openrouter_tests_shared_fixtures.md) - low (high review of `split_openrouter_tests`, 2026-10-29): test hygiene with no user-visible effect; next to `render_markdown_edge_cases`, same origin, before `git_log_follow_renames`
- [git_log_follow_renames](git_log_follow_renames.md) - not a priority (user, 2026-10-09): small MCP improvement, do it when nothing more valuable is left
- [smoke_warnings_business_paths_sources_and_empty_use_cases](smoke_warnings_business_paths_sources_and_empty_use_cases.md) - priority low; remeasure after the big changes
- [rerun_rewrites_files_with_a_timestamp](rerun_rewrites_files_with_a_timestamp.md) - priority low (user, 2026-10-16): the rerun of the same smoke writes 3 files for a `generated_at` stamp; cosmetic for the smoke criterion
- [merge_form_screens_into_their_use_case](merge_form_screens_into_their_use_case.md) - on hold (user, 2026-10-15): the measure found the real pairs at about 2 to 5% of a 52 use cases run; retake it on a complete run before deciding (see the issue)
