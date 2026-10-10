# Issue queue

Open issues in the order to work on them: the first is the next one. Read by `just next-issue`;
`just close-issue` removes a line, `skill:create-issue` inserts one at its rank, `skill:prioritize-issues`
re-ranks the whole list. Line format: `- [<file stem>](<file stem>.md) - <why here>`.

- [deterministic_confidence_checks](deterministic_confidence_checks.md) - second largest cost after the use cases; step 7 of ADR 0020
- [tree_sitter_structure](tree_sitter_structure.md) - structure map for the optional repo map of the behaviour issue; fewer chunking warnings
- [single_read_per_business_file](single_read_per_business_file.md) - step 6 of ADR 0020; before business_rules, which adds rules to the same reading
- [business_rules](business_rules.md) - ADR 0021, uses bounded entry points and structural detection from tree-sitter
- [docs_reconciliation_debt_report](docs_reconciliation_debt_report.md) - first PRODUCT.md feature, needs stable generated docs with rules
- [measure_model_split_per_pass](measure_model_split_per_pass.md) - after the pivot: it removes and changes passes, which would make an earlier split measure stale
- [brief_injection_experiments](brief_injection_experiments.md) - only once the pipeline stopped moving (behaviour issue) and with a steadier measure
- [surface_domains_default_decision](surface_domains_default_decision.md) - parked (user, 2026-10-10): an optimization; the default stays `files` until the cost of more benchmark runs is acceptable
- [git_log_follow_renames](git_log_follow_renames.md) - not a priority (user, 2026-10-09): small MCP improvement, do it when nothing more valuable is left
- [smoke_warnings_business_paths_sources_and_empty_use_cases](smoke_warnings_business_paths_sources_and_empty_use_cases.md) - priority low; remeasure after the big changes
