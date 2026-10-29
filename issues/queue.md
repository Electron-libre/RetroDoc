# Issue queue

Open issues in the order to work on them: the first is the next one. Read by `just next-issue`;
`just close-issue` removes a line, `skill:create-issue` inserts one at its rank, `skill:prioritize-issues`
re-ranks the whole list. Line format: `- [<file stem>](<file stem>.md) - <why here>`.

- [shared_lossy_read_and_one_pass_truncation](shared_lossy_read_and_one_pass_truncation.md) - small, right after `remove_repo_map` (its review found it): before `pass_runner_and_run_context` refactors the same passes
- [pass_runner_and_run_context](pass_runner_and_run_context.md) - after `single_pipeline_path`: do not refactor code about to be removed
- [remove_sub_domains](remove_sub_domains.md) - after `pass_runner_and_run_context`: dead code left by ADR 0035 (only the removed `files` path made sub-domains); wide but mechanical, touches the render and the MCP, which nothing before it does
- [contexts_residual_size_and_names](contexts_residual_size_and_names.md) - low: follow-up of the closed split by family (2026-10-14): one context still over 150 files, action names taken for families, the names unread; nothing before it depends on it, and its spread measure is cheaper after `sampling_temperature_for_steadier_runs`
- [rules_replay_without_evidence](rules_replay_without_evidence.md) - small, right before `rules_plumbing_and_screen_noise`: its rules measures should replay the pass with the evidence `generate` gives
- [rules_plumbing_and_screen_noise](rules_plumbing_and_screen_noise.md) - low (user, 2026-10-23): fine tuning of rules that are already usable; each step gained less than the one before, so after `contexts_residual_size_and_names` and well behind the structural work
- [smoke_run_keeps_previous_clone](smoke_run_keeps_previous_clone.md) - low (code review of the ledger issue, 2026-10-27): the skill's copy rule covers it until then; before `git_log_follow_renames`, it protects the saved runs the planned measures read
- [git_log_follow_renames](git_log_follow_renames.md) - not a priority (user, 2026-10-09): small MCP improvement, do it when nothing more valuable is left
- [smoke_warnings_business_paths_sources_and_empty_use_cases](smoke_warnings_business_paths_sources_and_empty_use_cases.md) - priority low; remeasure after the big changes
- [rerun_rewrites_files_with_a_timestamp](rerun_rewrites_files_with_a_timestamp.md) - priority low (user, 2026-10-16): the rerun of the same smoke writes 3 files for a `generated_at` stamp; cosmetic for the smoke criterion
- [merge_form_screens_into_their_use_case](merge_form_screens_into_their_use_case.md) - on hold (user, 2026-10-15): the measure found the real pairs at about 2 to 5% of a 52 use cases run; retake it on a complete run before deciding (see the issue)
